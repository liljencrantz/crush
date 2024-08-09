use std::collections::HashSet;
use crate::lang::command::CrushCommand;
use crate::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;
use crate::lang::command::OutputType::Unknown;
use chrono::Duration;
use crate::data::r#struct::Struct;
use crate::lang::state::scope::Scope;
use std::process;
use std::process::Stdio;
use std::io::Read;
use crossbeam::channel::bounded;
use crate::lang::data::list::List;
use crate::lang::data::table::{Row, Table};
use crate::lang::errors::{error, mandate};
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::this::This;
use crate::builtins::io::json::{json_to_value, value_to_json};
use crate::lang::value::ValueType;
use crate::lang::data::table::ColumnType;

#[signature(
    grpc.connect,
    can_block = true,
    short = "Create a connection to a gRPC service)",
    long = "This command currently does not currently do what it says. It's a proof of concept that\n    uses grpcurl under the hood. It does not have presistent connections and is quite slow and unreliable."
)]
struct Connect {
    #[description("Host to connect to.")]
    host: String,
    #[description("Service to connect to on this host")]
    service: Patterns,
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
                                    host: host.to_string(),
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

    fn call<S: Into<String>>(&self, context: &CommandContext, data: Option<String>, mut args: Vec<S>) -> CrushResult<String> {
        let mut cmd = process::Command::new("grpcurl");

        if self.plaintext {
            cmd.arg("--plaintext");
        }

        cmd.arg("--max-time").arg(self.timeout.num_seconds().to_string());

        if let Some(data) = data {
            cmd.arg("-d").arg(data);
        }

        cmd.arg(format!("{}:{}", self.host, self.port));
        for a in args.drain(..) {
            cmd.arg::<String>(a.into());
        }

        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());

        let mut child = cmd.spawn()?;

        let mut stdout = mandate(child.stdout.take(), "Expected output stream")?;
        let mut buff = Vec::new();
        stdout.read_to_end(&mut buff)?;
        let output = String::from_utf8(buff)?;
        let (send_err, recv_err) = bounded(1);
        let mut stderr = mandate(child.stderr.take(), "Expected error stream")?;
        context.spawn("grpcurl:stderr", move || {
            let mut buff = Vec::new();
            stderr.read_to_end(&mut buff)?;
            let errors = String::from_utf8(buff)?;
            let _ = send_err.send(errors);
            Ok(())
        })?;

        match child.wait()?.success() {
            true => Ok(output),
            false => argument_error_legacy(recv_err.recv()?),
        }
    }
}

fn connect(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Connect = Connect::parse(context.remove_arguments(), &context.global_state.printer())?;

    let tmp = Struct::new(
        vec![
            ("host", Value::from(cfg.host.clone())),
            ("plaintext", Value::Bool(cfg.plaintext)),
            ("timeout", Value::Duration(cfg.timeout)),
            ("port", Value::Integer(cfg.port)),
        ],
        None);

    let g = Grpc::new(Value::Struct(tmp))?;
    let s = Struct::from_vec(vec![], vec![]);
    let list = g.call(&context, None, vec!["list"])?;

    let mut available_services = list.lines().collect::<Vec<&str>>();
    let services = available_services.drain(..).filter(|s| { cfg.service.test(s) }).collect::<Vec<&str>>();

    for service in services {
        let out = g.call(&context, None, vec!["list", service])?;
        for line in out.lines() {
            let stripped = line.strip_prefix(&format!("{}.", service));
            if let Some(method) = stripped {
                s.set(
                    method, Value::Struct(
                        Struct::new(
                            vec![
                                ("host", Value::from(cfg.host.clone())),
                                ("service", Value::from(service.to_string())),
                                ("plaintext", Value::Bool(cfg.plaintext)),
                                ("timeout", Value::Duration(cfg.timeout)),
                                ("port", Value::Integer(cfg.port)),
                                ("method", Value::from(line)),
                                (
                                    "__call__",
                                    Value::Command(<dyn CrushCommand>::command(
                                        grpc_method_call,
                                        true,
                                        &["global", "grpc", "connect", method, "__call__"],
                                        "",
                                        "Call gRPC method",
                                        None,
                                        Unknown,
                                        [],
                                    )),
                                ),
                            ],
                            None,
                        )
                    ));
            }
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
                    return argument_error_legacy("gRPC method invocations can only use named arguments");
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
        let out =
            grpc.call(&context, data, vec![method.to_string()])?;

        let split = out.split("\n}\n{\n");

        let mut lst = split.into_iter()
            .map(|i| {
                let stripped = i.trim();
                match (stripped.starts_with("{"), stripped.ends_with("}")) {
                    (true, true) => json_to_value(i),
                    (true, false) => json_to_value(&format!("{}}}", i)),
                    (false, false) => json_to_value(&format!("{{{}}}", i)),
                    (false, true) => json_to_value(&format!("{{{}", i)),
                }
            })
            .collect::<CrushResult<Vec<_>>>()?;

        let types: HashSet<ValueType> = lst.iter().map(|v| v.value_type()).collect();
        let struct_types: HashSet<Vec<ColumnType>> = lst
            .iter()
            .flat_map(|v| match v {
                Value::Struct(r) => vec![r.local_signature()],
                _ => vec![],
            })
            .collect();

        let res = match types.len() {
            0 => Value::Empty,
            1 => {
                let list_type = types.iter().next().unwrap();
                match (list_type, struct_types.len()) {
                    (ValueType::Struct, 1) => {
                        let row_list = lst
                            .drain(..)
                            .map(|v| match v {
                                Value::Struct(r) => Ok(r.to_row()),
                                _ => error("Impossible!"),
                            })
                            .collect::<CrushResult<Vec<Row>>>()?;
                        Value::Table(Table::from((
                            struct_types.iter().next().unwrap().clone(),
                            row_list,
                        )))
                    }
                    _ => List::new(list_type.clone(), lst).into(),
                }
            }
            _ => List::new(ValueType::Any, lst).into(),
        };

        context.output.send(res)?;

        return Ok(());
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
