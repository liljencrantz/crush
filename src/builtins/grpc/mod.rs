use crate::CrushResult;
use crate::data::r#struct::Struct;
use crate::lang::any_str::AnyStr;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::{CrushCommand, Parameter};
use crate::lang::data::table::{ColumnType, TableReader};
use crate::lang::data::table::{Row, Table};
use crate::lang::errors::CrushError;
use crate::lang::errors::CrushErrorType::InvalidArgument;
use crate::lang::errors::command_error;
use crate::lang::pipe::Stream;
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use bytes::Buf;
use chrono::Duration;
use client::GrpcClient;
use itertools::Itertools;
use prost::Message as ProstMessage;
use prost_reflect::MethodDescriptor;
use signature::signature;
use std::sync::LazyLock;
use tokio::runtime::Runtime;
use tonic::codec::Codec;

mod client;
mod codec;

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to initialize static Tokio runtime")
});

#[signature(
    grpc.connect,
    can_block = true,
    short = "Create a connection to a gRPC service)",
    long = "This command currently uses grpcurl under the hood. It does not have a persistent gRPC connections and can therefore be slow."
)]
struct Connect {
    #[description("Host to connect to.")]
    host: String,

    #[description(
        "Service to connect to on this host. This can be a string, a glob or a regular expression, in order to allow you to easily specify multiple services, e.g. use `*` to connect to all available services."
    )]
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

fn runtime() -> &'static Runtime {
    &RUNTIME
}

fn connect(context: CommandContext) -> CrushResult<()> {
    runtime().block_on(connect_async(context))
}

async fn connect_async(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Connect = Connect::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.service.is_empty() {
        return command_error(
            "You must specify at least one service to connect to. You can use globs, such as `*`.",
        );
    }

    let id = GrpcClient::create(&cfg.host, cfg.plaintext, cfg.timeout, cfg.port).await?;
    let grpc_client = GrpcClient::get_from_id(id)?;
    let grpc_struct = Struct::from_vec(vec![], vec![]);
    let all = grpc_client.list_services().await?;
    let filtered = all
        .iter()
        .map(|s| s.name.clone())
        .filter(|s| cfg.service.test(&s))
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        return command_error(format!(
            "No match for service pattern `{}`. Found the following services: {}.",
            cfg.service.to_string(),
            all.iter().map(|s| s.name.clone()).join(", ")
        ));
    }

    grpc_struct.set(
        "close",
        Value::Struct(Struct::new(
            vec![
                ("id", Value::from(id)),
                (
                    "__call__",
                    Value::Command(<dyn CrushCommand>::command(
                        grpc_close_call,
                        true,
                        &["global", "grpc", "connect", "close", "__call__"],
                        format!("close"),
                        "Close this gRPC connection and release all related resources",
                        None::<AnyStr>,
                        Known(ValueType::Empty),
                        [],
                    )),
                ),
            ],
            None,
        )),
    );

    for service in &filtered {
        let out = grpc_client.list_methods(service).await?;
        for method in out.lines() {
            let stripped = method.strip_prefix(&format!("{}.", service));
            if let Some(method) = stripped {
                let signature = grpc_client.describe_method(service, method).await?;
                let input = signature.input();

                let signature_str = input
                    .fields()
                    .map(|field| {
                        format!(
                            "{}={}",
                            field.name(),
                            client::crush_type(field.kind()).to_string()
                        )
                    })
                    .join(" ");

                grpc_struct.set(
                    method,
                    Value::Struct(Struct::new(
                        vec![
                            ("id", Value::from(id)),
                            ("method", Value::from(method)),
                            ("service", Value::from(service)),
                            (
                                "__call__",
                                Value::Command(<dyn CrushCommand>::command(
                                    grpc_method_call,
                                    true,
                                    &["global", "grpc", "connect", method, "__call__"],
                                    format!("{} {}", method, signature_str),
                                    format!(
                                        "Call the {} method of the {} gRPC service",
                                        method, service
                                    ),
                                    Some(generate_long_help(&signature)),
                                    Unknown,
                                    generate_parameters(&signature),
                                )),
                            ),
                        ],
                        None,
                    )),
                );
            }
        }
    }
    context.output.send(Value::Struct(grpc_struct))
}

fn generate_parameters(signature: &MethodDescriptor) -> Vec<Parameter> {
    signature
        .input()
        .fields()
        .map(|field| Parameter {
            name: field.name().to_string(),
            value_type: ValueType::String,
            default: None,
            allowed: None,
            description: None,
            complete: None,
            named: false,
            unnamed: false,
        })
        .collect()
}

fn generate_long_help(signature: &MethodDescriptor) -> String {
    let mut res = String::new();
    if signature.input().fields().len() > 0 {
        res += "This command accepts the following arguments:\n\n";
        for field in signature.input().fields() {
            res += format!(
                "* `{}` ({})",
                field.name(),
                client::crush_type(field.kind()).to_string()
            )
            .as_str();
        }
    }
    res
}

fn grpc_method_call(context: CommandContext) -> CrushResult<()> {
    runtime().block_on(grpc_method_call_async(context))
}

async fn grpc_method_call_async(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let grpc_client = GrpcClient::get(&this)?;

    match (this.get("service"), this.get("method")) {
        (Some(Value::String(service)), Some(Value::String(method))) => {
            let data: Stream = if context.input.is_pipeline() {
                context.input.recv()?.stream(context.command_handle())?
            } else {
                if !context.arguments.is_empty() {
                    let mut fields = vec![];
                    let mut input_signature = vec![];

                    let signature = grpc_client.describe_method(&service, &method).await?;
                    let input_type = signature.input();
                    for a in context.remove_arguments() {
                        if let Some(name) = a.argument_type {
                            let field_type =
                                input_type.get_field_by_name(&name).ok_or_else(|| {
                                    CrushError::from(InvalidArgument(format!(
                                        "Unknown field `{}`",
                                        name
                                    )))
                                })?;

                                input_signature
                                    .push(ColumnType::new_from_string(name, a.value.value_type()));
                                fields.push(a.value);
                        } else {
                            return command_error(
                                "gRPC method invocations can only use named arguments.",
                            );
                        }
                    }
                    Box::from(TableReader::new(Table::from((
                        input_signature,
                        vec![Row::new(fields)],
                    ))))
                } else {
                    panic!()
                }
            };

            grpc_client
                .invoke_method(
                    &service,
                    &method,
                    data,
                    context.output,
                    context.global_state.printer(),
                )
                .await
        }
        _ => command_error("Invalid method field."),
    }
}

fn grpc_close_call(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    match this.get("id") {
        Some(Value::Integer(id)) => {
            GrpcClient::close(id as i32);
            context.output.send(Value::Empty)
        }
        _ => command_error("Invalid method id"),
    }
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
