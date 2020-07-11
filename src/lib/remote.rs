use crate::lang::errors::{CrushResult, to_crush_error, CrushError, mandate, error};
use crate::lang::value::Value;
use crate::lang::scope::Scope;
use crate::lang::execution_context::{ExecutionContext};
use signature::signature;
use crate::lang::command::Command;
use crate::lang::argument::ArgumentHandler;
use std::net::TcpStream;
use std::io::{Read, Write};
use std::cmp::min;
use ssh2::Session;
use crate::lang::serialization::{serialize, deserialize};
use std::thread;
use crossbeam::unbounded;
use crate::lang::table::{ColumnType, Row};
use crate::lang::value::ValueType;
use users::get_current_username;
use std::thread::JoinHandle;

#[signature(
exec,
can_block = true,
short = "Execute a command on a set of hosts",
long = "    Execute the specified command all specified hosts")]
struct Exec {
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
}

fn exec(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Exec = Exec::parse(context.arguments, &context.printer)?;
    let cmd = Value::Command(cfg.command);

    let (host_send, host_recv) = unbounded::<String>();
    let (result_send, result_recv) = unbounded::<(String, Value)>();

    let mut in_buf = Vec::new();

    serialize(&cmd, &mut in_buf)?;

    for host in &cfg.host {
        to_crush_error(host_send.send(host.clone()))?;
    }

    drop(host_send);

    let thread_count = min(cfg.parallel as usize, cfg.host.len());
    let mut threads: Vec<JoinHandle<std::result::Result<(), CrushError>>> = Vec::with_capacity(thread_count);
    for _ in 0..thread_count {
        let my_recv = host_recv.clone();
        let my_send = result_send.clone();
        let my_buf = in_buf.clone();
        let my_env = context.env.clone();
        let my_username =
            cfg.username.clone().unwrap_or(
                mandate(
                    mandate(
                        get_current_username(),
                        "Could not determine current username")?.to_str(),
                    "Invalid username")?.to_string());

        let t: JoinHandle<std::result::Result<(), CrushError>> =
            to_crush_error(thread::Builder::new().name("remote:exec".to_string()).spawn(
                move || {
                    while let Ok(mut host) = my_recv.recv() {
                        let username;
                        if host.contains('@') {
                            let mut tmp = host.splitn(2, '@');
                            username = tmp.next().unwrap().to_string();
                            host = tmp.next().unwrap().to_string();
                        } else {
                            username = my_username.clone();
                        }

                        if !host.contains(':') {
                            host = format!("{}:22", host);
                        }
                        let tcp = to_crush_error(TcpStream::connect(&host))?;
                        let mut sess = to_crush_error(Session::new())?;

                        sess.set_tcp_stream(tcp);
                        to_crush_error(sess.handshake())?;
                        to_crush_error(sess.userauth_agent(&username))?;

                        let mut channel = to_crush_error(sess.channel_session())?;
                        to_crush_error(channel.exec("crush --pup"))?;
                        to_crush_error(channel.write(&my_buf))?;
                        to_crush_error(channel.send_eof())?;
                        let mut out_buf = Vec::new();
                        to_crush_error(channel.read_to_end(&mut out_buf))?;
                        let res = deserialize(&out_buf, &my_env)?;
                        to_crush_error(channel.wait_close())?;
                        to_crush_error(my_send.send((host, res)))?;
                    }
                    Ok(())
                }))?;
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
            Ok(res) => {res?;},
            Err(_) => {return error("Unknown error while waiting for thread in remote:exec")},
        }
    }

    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_lazy_namespace(
        "remote",
        Box::new(move |env| {
            Exec::declare(env)?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
