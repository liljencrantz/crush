use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;
use crate::{argument_error_legacy, CrushResult, to_crush_error};
use crate::lang::execution_context::{CommandContext, This};
use crate::lang::value::{Symbol, Value, ValueType};
use signature::signature;
use crate::data::table::{ColumnType, Row};
use crate::lang::command::OutputType::{Known, Unknown};
use chrono::Duration;
use crate::data::r#struct::Struct;
use crate::data::scope::Scope;
use std::process;
use std::process::Stdio;
use std::io::{Write, Read};
use crate::lang::errors::{argument_error, mandate};
use crate::lib::io::json::{json_to_value, value_to_json};

#[signature(
connect,
can_block = true,
short = "Create a connection to a gRPC service)",
long = "This command currently does not currently do what it says. It's a proof of concept that\n    uses grpcurl under the hood. It does not have presistent connections and is quite slow and unreliable."
)]
struct Connect {
    #[description("Host to connect to.")]
    host: String,
    #[description("Service to connect to on this host")]
    service: String,
    #[default(false)]
    plaintext: bool,
    #[default(Duration::seconds(10))]
    timeout: Duration,
    #[default(5990)]
    port: i128,
}

struct Grpc {
    host: String,
    plaintext: bool,
    timeout: Duration,
    port: i128,
}

impl Grpc {
    fn new(v: Value) -> CrushResult<Grpc> {
        match v {
            Value::Struct(s) => {
                if let Some(Value::String(host)) = s.get("host") {
                    if let Some(Value::Bool(plaintext)) = s.get("plaintext") {
                        if let Some(Value::Duration(timeout)) = s.get("timeout") {
                            if let Some(Value::Integer(port)) = s.get("port") {
                                return Ok(Grpc {
                                    host,
                                    plaintext,
                                    timeout,
                                    port,
                                });
                            }
                        }
                    }
                }
                argument_error_legacy("Invalid struct specification")
            }
            _ => argument_error_legacy("Expected a struct"),
        }
    }

    fn call(&self, data: Option<String>, args: &[String]) -> CrushResult<(bool, String, String)> {
        let mut cmd = process::Command::new("grpcurl");

        if self.plaintext {
            cmd.arg("--plaintext");
        }

        cmd.arg("--max-time").arg(self.timeout.num_seconds().to_string());

        if let Some(data) = data {
            cmd.arg("-d").arg(data);
        }

        cmd.arg(format!("{}:{}", self.host, self.port));
        for a in args {
            cmd.arg(a);
        }


        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = to_crush_error(cmd.spawn())?;

        let mut stdout = mandate(child.stdout.take(), "Expected output stream")?;
        //  context.spawn("grpcurl:stdout", move || {
        let mut buff = Vec::new();
        to_crush_error(stdout.read_to_end(&mut buff))?;
        let output = to_crush_error(String::from_utf8(buff))?;
        //    Ok(())
        //})?;
        /*
    let mut stderr = mandate(child.stderr.take(), "Expected error stream")?;
    context.spawn("grpcurl:stderr", move || {
        let mut buff = Vec::new();
        to_crush_error(stderr.read_to_end(&mut buff))?;
        let errors = to_crush_error(String::from_utf8(buff))?;
        Ok(())
    })?;
*/
        let res = child.wait()?.success();
        Ok((res, output, "".to_string()))
    }
}

fn list_services(grpc_data: &Grpc) -> CrushResult<Vec<String>> {
    todo!()
}

fn list_methods(grpc_data: &Grpc, service: &str) -> CrushResult<Vec<String>> {
    todo!()
}


fn connect(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Connect = Connect::parse(context.remove_arguments(), &context.global_state.printer())?;

    let tmp = Struct::new(
        vec![
            ("host".to_string(), Value::String(cfg.host.clone())),
            ("service".to_string(), Value::String(cfg.service.clone())),
            ("plaintext".to_string(), Value::Bool(cfg.plaintext)),
            ("timeout".to_string(), Value::Duration(cfg.timeout)),
            ("port".to_string(), Value::Integer(cfg.port)),
        ],
        None);

    let g = Grpc::new(Value::Struct(tmp))?;
    let (ok, out, err) = g.call(None, &vec!["list".to_string(), cfg.service.clone()])?;
    if !ok {
        return argument_error_legacy(err);
    }
    let s = Struct::from_vec(vec![], vec![]);

    for line in out.lines() {
        let stripped = line.strip_prefix(&format!("{}.", &cfg.service));
        if let Some(method) = stripped {
            s.set(method, Value::Struct(
                Struct::new(
                    vec![
                        ("host".to_string(), Value::String(cfg.host.clone())),
                        ("service".to_string(), Value::String(cfg.service.clone())),
                        ("plaintext".to_string(), Value::Bool(cfg.plaintext)),
                        ("timeout".to_string(), Value::Duration(cfg.timeout)),
                        ("port".to_string(), Value::Integer(cfg.port)),
                        ("method".to_string(), Value::string(line)),
                        (
                            "__call__".to_string(),
                            Value::Command(<dyn CrushCommand>::command(
                                grpc_method_call,
                                true,
                                vec![
                                    "global".to_string(),
                                    "grpc".to_string(),
                                    "connect".to_string(),
                                    method.to_string(),
                                    "__call__".to_string(),
                                ],
                                "",
                                "Call gRPC method",
                                None,
                                Unknown,
                                vec![],
                            )),
                        ),
                    ],
                    None)
            ));
        }
    }
    context.output.send(Value::Struct(s))
}

fn grpc_method_call(mut context: CommandContext) -> CrushResult<()> {
    let data = if context.input.is_pipeline() {
        let data = context.input.recv()?;
        Some(value_to_json(data)?)
    } else {
        if !context.arguments.is_empty() {
            let mut fields = Vec::new();
            for a in context.remove_arguments() {
                if let Some(name) = a.argument_type {
                    fields.push((name, a.value));
                } else {
                    return argument_error_legacy("gRPC method invocations can only use named arguments")
                }
            }
            let s = Struct::new(
                fields,
                None,
            );
            Some(value_to_json(Value::Struct(s))?)
        } else {
            None
        }
    };
    let this = context.this.r#struct()?;
    if let Some(Value::String(method)) = this.get("method") {
        let grpc = Grpc::new(Value::Struct(this))?;
        let (ok, out, err) =
            grpc.call(data, &vec![method])?;
        return context.output.send(json_to_value(&out)?);
    }
    return argument_error_legacy("Invalid method field");
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "grpc",
        "gRPC connection",
        Box::new(move |grpc| {
            Connect::declare(grpc)?;
            Ok(())
        }))?;
    Ok(())
}
