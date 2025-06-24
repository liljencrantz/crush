use crate::builtins::io::json::{json_to_value, value_to_json};
use crate::data::r#struct::Struct;
use crate::lang::any_str::AnyStr;
use crate::lang::command::CrushCommand;
use crate::lang::command::OutputType::Unknown;
use crate::lang::data::list::List;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::{Row, Table};
use crate::lang::errors::error;
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::{CrushResult, argument_error_legacy};
use chrono::Duration;
use crossbeam::channel::bounded;
use itertools::Itertools;
use regex::Regex;
use signature::signature;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::process;
use std::process::Stdio;
use std::sync::OnceLock;

#[signature(
    grpc.connect,
    can_block = true,
    short = "Create a connection to a gRPC service)",
    long = "This command currently uses grpcurl under the hood. It does not have a persistent gRPC connections and can therefore be slow."
)]
struct Connect {
    #[description("Host to connect to.")]
    host: String,
    
    #[description("Service to connect to on this host. This can be a string, a glob or a regular expression, in order to allow you to easily specify multiple services, e.g. use `*` to connect to all available services.")]
    service: Patterns,
    
    #[default(false)]
    #[description("Use plaintext to connect")]
    plaintext: bool,
    
    #[default(Duration::seconds(5))]
    #[description("Timeout for making calls")]
    timeout: Duration,
    
    #[default(5990)]
    #[description("Port to connect to")]
    port: i128,
}

struct Grpc {
    host: String,
    plaintext: bool,
    timeout: Duration,
    port: i128,
}

impl Grpc {
    fn new(s: Struct) -> CrushResult<Grpc> {
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

    fn call<S: Into<String>>(
        &self,
        context: &CommandContext,
        data: Option<String>,
        mut args: Vec<S>,
    ) -> CrushResult<String> {
        let mut cmd = process::Command::new("grpcurl");

        if self.plaintext {
            cmd.arg("--plaintext");
        }

        cmd.arg("--max-time")
            .arg(self.timeout.num_seconds().to_string());

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

        let mut stdout = child.stdout.take().ok_or("Expected output stream")?;
        let mut buff = Vec::new();
        stdout.read_to_end(&mut buff)?;
        let output = String::from_utf8(buff)?;
        let (send_err, recv_err) = bounded(1);
        let mut stderr = child.stderr.take().ok_or("Expected error stream")?;
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

#[derive(Clone, Debug)]
struct ProtoMessage {
    name: String,
    fields: Vec<ProtoField>,
}

#[derive(Clone, Debug)]
struct ProtoField {
    name: String,
    proto_type: ProtoType,
}

#[derive(Clone, Debug)]
enum ProtoType {
    Int64,
    UInt64,
    Int32,
    UInt32,
    Double,
    Float,
    Bool,
    String,
    Bytes,
    Message(ProtoMessage),
}

impl ProtoType {
    fn crush_type(&self) -> ValueType {
        match self {
            ProtoType::Int64 => ValueType::Integer,
            ProtoType::UInt64 => ValueType::Integer,
            ProtoType::Int32 => ValueType::Integer,
            ProtoType::UInt32 => ValueType::Integer,
            ProtoType::Double => ValueType::Float,
            ProtoType::Float => ValueType::Float,
            ProtoType::Bool => ValueType::Bool,
            ProtoType::String => ValueType::String,
            ProtoType::Bytes => ValueType::Binary,
            ProtoType::Message(_) => ValueType::Struct,
        }
    }

    fn arguments(&self) -> String {
        if let ProtoType::Message(fields) = self {
            fields
                .fields
                .iter()
                .map(|f| format!("{}={}", f.name, f.proto_type.crush_type().to_string()))
                .join(" ")
        } else {
            self.crush_type().to_string()
        }
    }
}

fn insert_known_types(known_types: &mut HashMap<String, ProtoType>) {
    known_types.insert("int32".to_string(), ProtoType::Int32);
    known_types.insert("int64".to_string(), ProtoType::Int64);
    known_types.insert("uint32".to_string(), ProtoType::UInt32);
    known_types.insert("uint64".to_string(), ProtoType::UInt64);
    known_types.insert("bool".to_string(), ProtoType::Bool);
    known_types.insert("string".to_string(), ProtoType::String);
    known_types.insert("bytes".to_string(), ProtoType::Bytes);
    known_types.insert("double".to_string(), ProtoType::Double);
    known_types.insert("float".to_string(), ProtoType::Float);
}

fn parse_message_type<'a>(
    context: &CommandContext,
    name: &str,
    grpc: &Grpc,
    known_types: &'a mut HashMap<String, ProtoType>,
) -> CrushResult<ProtoType> {
    if let Some(t) = known_types.get(name) {
        return Ok(t.clone());
    }

    let signature = grpc.call(context, None, vec!["describe", name])?;

    static REGEX: OnceLock<Regex> = OnceLock::new();
    let re = REGEX.get_or_init(|| {
        Regex::new(r"[[:blank:]]*([a-zA-Z_.][a-zA-Z0-9_.]*)[[:blank:]]+([a-zA-Z_][a-zA-Z0-9_]*)[[:blank:]]*=[[:blank:]]*([0-9]+);[[:blank:]]*").unwrap()
    });

    let mut fields = Vec::new();

    for line in signature.lines() {
        match re.captures(line) {
            None => {}
            Some(c) => match (c.get(1), c.get(2)) {
                (Some(type_name), Some(field_name)) => {
                    let field_type =
                        parse_message_type(context, type_name.as_str(), grpc, known_types)?;
                    fields.push(ProtoField {
                        name: field_name.as_str().to_string(),
                        proto_type: field_type,
                    });
                }
                _ => {}
            },
        };
    }

    let res = ProtoType::Message(ProtoMessage {
        name: name.to_string(),
        fields,
    });

    known_types.insert(name.to_string(), res.clone());

    Ok(res)
}

fn connect(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Connect = Connect::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.service.is_empty() {
        return argument_error_legacy(
            "You must specify at least one service to connect to. You can use globs, such as '*'",
        );
    }
    let tmp = Struct::new(
        vec![
            ("host", Value::from(cfg.host.clone())),
            ("plaintext", Value::Bool(cfg.plaintext)),
            ("timeout", Value::Duration(cfg.timeout)),
            ("port", Value::Integer(cfg.port)),
        ],
        None,
    );

    let g = Grpc::new(tmp)?;
    let s = Struct::from_vec(vec![], vec![]);
    let list = g.call(&context, None, vec!["list"])?;
    let mut available_services = list.lines().collect::<Vec<&str>>();
    let services = available_services
        .drain(..)
        .filter(|s| cfg.service.test(s))
        .collect::<Vec<&str>>();

    if services.is_empty() {
        return argument_error_legacy(format!(
            "No match for service pattern {}. Found services {}",
            cfg.service.to_string(),
            list.lines().join(", ")
        ));
    }

    let mut known_types = HashMap::new();
    insert_known_types(&mut known_types);

    for service in services {
        let out = g.call(&context, None, vec!["list", service])?;
        for line in out.lines() {
            let stripped = line.strip_prefix(&format!("{}.", service));
            if let Some(method) = stripped {
                let signature = g.call(
                    &context,
                    None,
                    vec!["describe".to_string(), format!("{}.{}", service, method)],
                )?;
                let input_type_name = parse_input_type_from_signature(method, signature.as_str())?;
                println!("{:?}", input_type_name);
                let input_type =
                    parse_message_type(&context, &input_type_name, &g, &mut known_types)?;
                println!("{:?}", input_type);

                s.set(
                    method,
                    Value::Struct(Struct::new(
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
                                    format!("{} {}", method, input_type.arguments()),
                                    format!(
                                        "Call the {} method of the {} service",
                                        method, service
                                    ),
                                    None::<AnyStr>,
                                    Unknown,
                                    [],
                                )),
                            ),
                        ],
                        None,
                    )),
                );
            }
        }
    }
    context.output.send(Value::Struct(s))
}

fn parse_input_type_from_signature<'a>(
    method_name: &str,
    signature: &'a str,
) -> CrushResult<&'a str> {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    let re = REGEX.get_or_init(|| Regex::new(r"\((.*)\).*\(.*\)").unwrap());
    for line in signature.lines() {
        if line.starts_with("rpc") {
            return match re.captures(line) {
                None => argument_error_legacy("Failed to parse signature"),
                Some(c) => match c.get(1) {
                    None => argument_error_legacy("Failed to parse signature"),
                    Some(m) => Ok(m.as_str().trim()),
                },
            };
        }
    }
    argument_error_legacy(format!(
        "Failed to parse signature of method {}",
        method_name
    ))
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
                    return argument_error_legacy(
                        "gRPC method invocations can only use named arguments",
                    );
                }
            }
            let s = Struct::new(fields, None);
            Some(value_to_json(Value::Struct(s))?)
        } else {
            None
        }
    };
    let this = context.this.r#struct()?;
    if let Some(Value::String(method)) = this.get("method") {
        let grpc = Grpc::new(this)?;
        let out = grpc.call(&context, data, vec![method.to_string()])?;

        let split = out.split("\n}\n{\n");

        let mut lst = split
            .into_iter()
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
        }),
    )?;
    Ok(())
}
