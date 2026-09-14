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
    short = "Create a connection to a gRPC service.",
    long = "gRPC (https://grpc.io) is Google's open-source, high-performance RPC",
    long = "framework, built on HTTP/2 and Protocol Buffers.",
    long = "",
    long = "To send RPC calls to a server, you must first create a grpc connection, writing something like `$conn := $(grpc:connect host=localhost service=* --plaintext)`.",
    long = "",
    long = "The resulting connection struct will have one method for each endpoint on the chosen services of the host you connected to. When calling a method, there are two different ways to pass in input parameters.",
    long = "",
    long = "If you want to pass exactly one message to the endpoint, e.g. because the endpoint does not use client streaming, you have the option of passing the fields of the message as arguments to the method call, e.g. `$conn:ReverseString input=hello`.",
    long = "The grpc methods support tab completion of argument names. They also come with help messages (e.g. `help $conn:ReverseString`) that describes the input and output format.",
    long = "",
    long = "If you want to pass multiple messages to the endpoint, you must do so by piping in a table_input_stream, where the column names of the stream are identical to the field names of the message, e.g. `list:of foo bar baz | select input={$value} | $conn:ReverseString`",
    long = "",
    long = "Output from a gRPC method call is always a `$table_input_stream`, with one row per message. If an endpoint does not use server side streaming, the output always has one row.",
    long = "",
    long = "Once you are done with a gRPC connection, you should close it to free up resources. Do so by calling the close method, e.g. `$conn:close`.",
    example = "$conn := $(grpc:connect host=localhost service=* --plaintext)",
    example = "# Returns \"olleh\"",
    example = "$conn:ReverseString input=\"hello\"",
    example = "# Returns a stream with the values \"oof\", \"rab\", and \"zab\"",
    example = "list:of foo bar baz | select input={$value} | $conn:ReverseString",
    example = "# Close the connection once you're done",
    example = "$conn:close",
)]
pub struct Connect {
    #[description("the host to connect to.")]
    host: String,

    #[description(
        "the service to connect to on this host. This can be a string, a glob or a regular expression, in order to allow you to easily specify multiple services, e.g. use `*` to connect to all available services."
    )]
    service: Patterns,

    #[default(false)]
    #[description("use plaintext instead of TLS to connect.")]
    plaintext: bool,

    #[default(Duration::seconds(5))]
    #[description("the timeout for making calls.")]
    timeout: Duration,

    #[default(50051)]
    #[description("the port to connect to.")]
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
            dirs_only: false,
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
