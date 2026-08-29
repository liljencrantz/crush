use crate::builtins::grpc;
use crate::builtins::grpc::client::GrpcClient;
use crate::lang::data::table::{ColumnType, Row, Table, TableReader};
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::pipe::Stream;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;

pub fn grpc_method_call(context: CommandContext) -> CrushResult<()> {
    grpc::runtime().block_on(grpc_method_call_async(context))
}

async fn grpc_method_call_async(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let grpc_client = GrpcClient::get(&this)?;

    match (this.get("service"), this.get("method")) {
        (Some(Value::String(service)), Some(Value::String(method))) => {
            let data: Stream = if context.input.is_pipeline() {
                context.input.recv()?.stream(context.command_handle())?
            } else {
                let mut fields = vec![];
                let mut input_signature = vec![];
                for a in context.remove_arguments() {
                    if let Some(name) = a.argument_type {
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