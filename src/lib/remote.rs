use crate::lang::errors::{CrushResult};
use crate::{
    lang::value::Value,
};
use crate::lang::scope::Scope;
use crate::lang::execution_context::{ExecutionContext};
use signature::signature;
use crate::lang::command::CommandWrapper;
use crate::lang::argument::ArgumentHandler;
use std::net::TcpStream;
use ssh2::Session;

fn exec(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Exec = Exec::parse(context.arguments, &context.printer)?;
    for host in &cfg.host {
        let tcp = TcpStream::connect(host).unwrap();
        let mut sess = Session::new().unwrap();
        sess.set_tcp_stream(tcp);
        sess.handshake().unwrap();
        sess.userauth_agent("liljencrantz").unwrap();
        assert!(sess.authenticated());
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
    command: CommandWrapper,
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
