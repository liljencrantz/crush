use crate::builtins::types::column_types;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Call::declare_method(&mut res);
        Write::declare_method(&mut res);

        res
    })
}

#[signature(
    types.table_output_stream.__call__,
    can_block = false,
    output = Known(ValueType::Type),
    short = "return the table_output_stream type with the specified column signature.",
)]
struct Call {
    #[description("the columns of the stream.")]
    #[named()]
    columns: OrderedStringMap<ValueType>,
}

fn __call__(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableOutputStream(c) => {
            let cfg: Call =
                Call::parse(context.remove_arguments(), &context.global_state.printer())?;
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableOutputStream(column_types(
                        &cfg.columns,
                    ))))
            } else if cfg.columns.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableOutputStream(c)))
            } else {
                command_error(
                    "Tried to set columns on a `table_output_stream` type that already has columns.",
                )
            }
        }
        _ => command_error("Invalid `this`, expected type `table_input_stream`."),
    }
}

#[signature(
    types.table_output_stream.write,
    output = Known(ValueType::Empty),
    short = "write input to this output stream",
)]
struct Write {}

fn write(mut context: CommandContext) -> CrushResult<()> {
    let real_output = context.this.table_output_stream()?;
    let mut stream = context.input.recv()?.stream()?;

    while let Ok(row) = stream.read() {
        real_output.send(row)?;
    }
    context.output.send(Value::Empty)
}
