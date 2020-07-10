use crate::lang::errors::{CrushResult, to_crush_error, CrushError};
use crate::{
    lang::value::Value,
};
use crate::lang::scope::Scope;
use crate::lang::execution_context::{ExecutionContext};
use signature::signature;
use crate::lang::command::Command;
use crate::lang::argument::ArgumentHandler;
use std::net::TcpStream;
use ssh2::Session;
use std::io::{Read, Write};
use crate::lang::serialization::{serialize, deserialize};
use std::cmp::min;
use std::thread;
use crossbeam::unbounded;
use crate::lang::table::{ColumnType, Row};
use crate::lang::value::ValueType;

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

    for _ in 0..(min(cfg.parallel as usize, cfg.host.len())) {
        let my_recv = host_recv.clone();
        let my_send = result_send.clone();
        let my_buf = in_buf.clone();
        let my_env = context.env.clone();
        let t: std::thread::JoinHandle<std::result::Result<(), CrushError>> =
            to_crush_error(thread::Builder::new().name("remote:exec".to_string()).spawn(
                move || {
                    while let Ok(host) = my_recv.recv() {
                        let tcp = to_crush_error(TcpStream::connect(&host))?;
                        let mut sess = to_crush_error(Session::new())?;

                        sess.set_tcp_stream(tcp);
                        to_crush_error(sess.handshake())?;
                        to_crush_error(sess.userauth_agent("liljencrantz"))?;

                        let mut channel = to_crush_error(sess.channel_session())?;
                        to_crush_error(channel.exec("/home/liljencrantz/src/crush/target/debug/crush --pup"))?;
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
    }

    drop(result_send);
    let output = context.output.initialize(vec![
        ColumnType::new("host", ValueType::String),
        ColumnType::new("result", ValueType::Any),
    ])?;

    while let Ok((host, val)) = result_recv.recv() {
        output.send(Row::new(vec![Value::String(host), val]))?;
    }

    Ok(())
}

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
