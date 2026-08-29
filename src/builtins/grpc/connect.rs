use crate::builtins::grpc;
use crate::builtins::grpc::client::GrpcClient;
use crate::builtins::grpc::{client, method_call};
use crate::lang::any_str::AnyStr;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::{CrushCommand, Parameter};
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::{Value, ValueType};
use chrono::Duration;
use itertools::Itertools;
use prost_reflect::MethodDescriptor;
use signature::signature;
use crate::lang::state::this::This;

#[signature(
    grpc.connect,
    can_block = true,
    short = "Create a connection to a gRPC service)",
    long = "This command currently uses grpcurl under the hood. It does not have a persistent gRPC connections and can therefore be slow."
)]
pub struct Connect {
    #[description("Host to connect to.")]
    host: String,

    #[description(
        "Service to connect to on this host. This can be a string, a glob or a regular expression, in order to allow you to easily specify multiple services, e.g. use `*` to connect to all available services."
    )]
    service: Patterns,

    #[default(false)]
    #[description("Use plaintext to connect.")]
    plaintext: bool,

    #[default(Duration::seconds(5))]
    #[description("Timeout for making calls.")]
    timeout: Duration,

    #[default(50051)]
    #[description("Port to connect to.")]
    port: i128,
}


fn connect(context: CommandContext) -> CrushResult<()> {
    grpc::runtime().block_on(connect_async(context))
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
                                    method_call::grpc_method_call,
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
                "* `{}` (`{}`)\n",
                field.name(),
                client::crush_type(field.kind()).to_string()
            )
            .as_str();
        }
    }
    res
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
