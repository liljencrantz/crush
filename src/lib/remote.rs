use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::errors::{error, mandate, to_crush_error, CrushError, CrushResult};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::scope::Scope;
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::table::{ColumnType, Row};
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crossbeam::unbounded;
use signature::signature;
use ssh2::Session;
use std::cmp::min;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::thread;
use std::thread::JoinHandle;
use users::get_current_username;

fn parse(mut host: String, default_username: &Option<String>) -> CrushResult<(String, String)> {
    let username;
    if host.contains('@') {
        let mut tmp = host.splitn(2, '@');
        username = tmp.next().unwrap().to_string();
        host = tmp.next().unwrap().to_string();
    } else {
        username = default_username.clone().unwrap_or(
            mandate(
                mandate(
                    get_current_username(),
                    "Could not determine current username",
                )?
                .to_str(),
                "Invalid username",
            )?
            .to_string(),
        );
    }

    if !host.contains(':') {
        host = format!("{}:22", host);
    }
    Ok((host, username))
}

fn run_remote(
    cmd: &Vec<u8>,
    env: &Scope,
    host: String,
    default_username: &Option<String>,
    password: &Option<String>,
) -> CrushResult<Value> {
    let (host, username) = parse(host, &default_username)?;

    let tcp = to_crush_error(TcpStream::connect(&host))?;
    let mut sess = to_crush_error(Session::new())?;

    sess.set_tcp_stream(tcp);
    to_crush_error(sess.handshake())?;
    if let Some(pass) = password {
        to_crush_error(sess.userauth_password(&username, pass))?
    } else {
        to_crush_error(sess.userauth_agent(&username))?;
    }

    let mut channel = to_crush_error(sess.channel_session())?;
    to_crush_error(channel.exec("crush --pup"))?;
    to_crush_error(channel.write(cmd))?;
    to_crush_error(channel.send_eof())?;
    let mut out_buf = Vec::new();
    to_crush_error(channel.read_to_end(&mut out_buf))?;
    let res = deserialize(&out_buf, env)?;
    to_crush_error(channel.wait_close())?;
    Ok(res)
}

#[signature(
    exec,
    can_block = true,
    short = "Execute a command on a host",
    long = "    Execute the specified command on the soecified host"
)]
struct Exec {
    #[description("the command to execute.")]
    command: Command,
    #[description("host to execute the command on.")]
    host: String,
    #[description("username on remote machines.")]
    username: Option<String>,
    #[description("password on remote machines. If no password is provided, agent authentication will be used.")]
    password: Option<String>,
}

fn exec(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Exec = Exec::parse(context.arguments, &context.printer)?;
    let mut in_buf = Vec::new();
    serialize(&Value::Command(cfg.command), &mut in_buf)?;
    context.output.send(run_remote(
        &in_buf,
        &context.env,
        cfg.host,
        &cfg.username,
        &cfg.username,
    )?)
}

#[signature(
    pexec,
    can_block = true,
    short = "Execute a command on a set of hosts",
    long = "    Execute the specified command all specified hosts"
)]
struct Pexec {
    #[description("the command to execute.")]
    command: Command,
    #[unnamed()]
    #[description("hosts to execute the command on.")]
    host: Vec<String>,
    #[description("maximum number of hosts to run on in parallel.")]
    #[default(32)]
    parallel: i128,
    #[description("username on remote machines.")]
    username: Option<String>,
    #[description("password on remote machines. If no password is provided, agent authentication will be used.")]
    password: Option<String>,
}

fn pexec(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Pexec = Pexec::parse(context.arguments, &context.printer)?;

    let (host_send, host_recv) = unbounded::<String>();
    let (result_send, result_recv) = unbounded::<(String, Value)>();

    let mut in_buf = Vec::new();

    serialize(&Value::Command(cfg.command), &mut in_buf)?;

    for host in &cfg.host {
        to_crush_error(host_send.send(host.clone()))?;
    }

    drop(host_send);

    let thread_count = min(cfg.parallel as usize, cfg.host.len());
    let mut threads: Vec<JoinHandle<std::result::Result<(), CrushError>>> =
        Vec::with_capacity(thread_count);
    for _ in 0..thread_count {
        let my_recv = host_recv.clone();
        let my_send = result_send.clone();
        let my_buf = in_buf.clone();
        let my_env = context.env.clone();
        let my_username = cfg.username.clone();
        let my_password = cfg.password.clone();

        let t: JoinHandle<std::result::Result<(), CrushError>> = to_crush_error(
            thread::Builder::new()
                .name("remote:pexec".to_string())
                .spawn(move || {
                    while let Ok(host) = my_recv.recv() {
                        let res =
                            run_remote(&my_buf, &my_env, host.clone(), &my_username, &my_password)?;
                        to_crush_error(my_send.send((host, res)))?;
                    }
                    Ok(())
                }),
        )?;
        threads.push(t);
    }

    drop(result_send);
    let output = context.output.initialize(vec![
        ColumnType::new("host", ValueType::String),
        ColumnType::new("result", ValueType::Any),
    ])?;

    while let Ok((host, val)) = result_recv.recv() {
        output.send(Row::new(vec![Value::String(host), val]))?;
    }

    for t in threads {
        match t.join() {
            Ok(res) => {
                res?;
            }
            Err(_) => {
                return error("Unknown error while waiting for thread in remote:exec");
            }
        }
    }

    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_lazy_namespace(
        "remote",
        Box::new(move |env| {
            Exec::declare(env)?;
            Pexec::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
