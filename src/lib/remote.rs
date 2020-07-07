use crate::lang::errors::{CrushResult, to_crush_error};
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

fn exec(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Exec = Exec::parse(context.arguments, &context.printer)?;
    let cmd = Value::Command(cfg.command);
    for host in &cfg.host {
        let tcp = TcpStream::connect(host).unwrap();
        let mut sess = Session::new().unwrap();
        sess.set_tcp_stream(tcp);
        sess.handshake().unwrap();
        sess.userauth_agent("liljencrantz").unwrap();
        assert!(sess.authenticated());

        let mut channel = sess.channel_session().unwrap();
        channel.exec("/home/liljencrantz/src/crush/target/debug/crush --pup").unwrap();
        let mut in_buf = Vec::new();
        serialize(&cmd, &mut in_buf)?;
        to_crush_error(channel.write(&in_buf))?;
        to_crush_error(channel.send_eof())?;
        let mut out_buf = Vec::new();
        to_crush_error(channel.read_to_end(&mut out_buf))?;
        println!("YAY {} {}", out_buf.len(), channel.exit_status().unwrap());
        let res = deserialize(&mut out_buf, &context.env)?;
        to_crush_error(channel.wait_close())?;
        context.output.send(res)?;
    }
    Ok(())
}

#[signature(
    exec,
    can_block=true,
    short="Execute a command on a set of hosts",
    long="    Execute the specified command all specified hosts")]
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
