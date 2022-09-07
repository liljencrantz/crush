use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error_legacy, CrushResult, mandate};
use crate::lang::execution_context::This;
use crate::lang::value::ValueType;
use crate::lang::{execution_context::CommandContext, value::Value};
use crate::lib::types::{column_types};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::ordered_string_map::OrderedStringMap;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "table_output_stream"];
        Call::declare_method(&mut res, &path);
        Write::declare_method(&mut res, &path);
        res
    };
}

#[signature(
__call__,
can_block = false,
output = Known(ValueType::Type),
short = "return the table_output_stream type with the specified column signature.",
)]
struct Call {
    #[description("the columns of the stream.")]
    #[named()]
    columns: OrderedStringMap<ValueType>,
}

fn __call__(context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableOutputStream(c) => {
            let cfg: Call = Call::parse(context.arguments, &context.global_state.printer())?;
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableOutputStream(column_types(&cfg.columns))))
            } else if cfg.columns.is_empty() {
                context.output.send(Value::Type(ValueType::TableOutputStream(c)))
            } else {
                argument_error_legacy(
                    "Tried to set columns on a table_output_stream type that already has columns",
                )
            }
        }
        _ => argument_error_legacy("Invalid this, expected type table_input_stream"),
    }
}

#[signature(
write,
output = Known(ValueType::Empty),
short = "write input to this output stream",
)]
struct Write {}

fn write(context: CommandContext) -> CrushResult<()> {
    let real_output = context.this.table_output_stream()?;
    let mut stream = mandate(context.input.recv()?.stream()?, "Expected a stream")?;

    while let Ok(row) = stream.read() {
        real_output.send(row)?;
    }
    context.output.send(Value::Empty())?;
    Ok(())
}
