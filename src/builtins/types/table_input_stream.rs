use std::sync::OnceLock;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error_legacy, CrushResult, mandate};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use crate::builtins::types::column_types;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::any_str::AnyStr;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::streams;
use crate::lang::data::r#struct::Struct;
use crate::lang::command::CrushCommand;
use crate::lang::state::this::This;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Call::declare_method(&mut res);
        GetItem::declare_method(&mut res);
        Pipe::declare_method(&mut res);

        res
    })
}

pub fn close_value() -> &'static Value {
    static CELL: OnceLock<Value> = OnceLock::new();
    CELL.get_or_init(|| Value::Command(<dyn CrushCommand>::command(
        close, false,
        &["global", "types", "pipe", "close"],
        "pipe:close",
        "Close the specified pipe",
        None::<AnyStr>,
        Known(ValueType::Empty),
        [],
    )))
}

pub fn write_value() -> &'static Value {
    static CELL: OnceLock<Value> = OnceLock::new();
    CELL.get_or_init(|| Value::Command(<dyn CrushCommand>::command(
        write, true,
        &["global", "types", "pipe", "write"],
        "pipe:write",
        "Write sink for this pipe",
        None::<AnyStr>,
        Known(ValueType::Empty),
        [],
    )))
}

#[signature(
    types.table_input_stream.__call__,
    can_block = false,
    output = Known(ValueType::Type),
    short = "return the table_input_stream type with the specified column signature.",
)]
struct Call {
    #[description("the columns of the stream.")]
    #[named()]
    columns: OrderedStringMap<ValueType>,
}

fn __call__(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableInputStream(c) => {
            let cfg: Call = Call::parse(context.arguments, &context.global_state.printer())?;
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableInputStream(column_types(&cfg.columns))))
            } else if cfg.columns.is_empty() {
                context.output.send(Value::Type(ValueType::TableInputStream(c)))
            } else {
                argument_error_legacy(
                    "Tried to set columns on a table_input_stream type that already has columns",
                )
            }
        }
        _ => argument_error_legacy("Invalid this, expected type table_input_stream"),
    }
}

#[signature(
    types.table_input_stream.__getitem__,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Returns the specified row of the table stream as a struct.",
    example = "(ps)[4]"
)]
struct GetItem {
    index: i128,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.arguments, &context.global_state.printer())?;
    let o = context.this.table_input_stream()?;
    context.output.send(Value::Struct(o.get(cfg.index)?.into_struct(o.types())))
}

#[signature(
    types.table_input_stream.pipe,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Returns a struct containing a read end and a write end of a pipe of the specified type",
    example = "$pipe := $($(table_input_stream value=$integer):pipe)\n    $_1 := $(seq 100_000 | $pipe:write | bg)\n    $sum_job_id := $($pipe:read | sum | bg)\n    $pipe:close\n    $sum_job_id | fg"
)]
struct Pipe {}

fn pipe(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableInputStream(subtype) => {
            let (output, input) = streams(subtype);
            context.output.send(Value::Struct(Struct::new(
                vec![
                    ("read", Value::TableInputStream(input)),
                    ("output", Value::TableOutputStream(output)),
                    ("write", write_value().clone()),
                    ("close", close_value().clone()),
                ],
                None,
            )))
        }
        _ => argument_error_legacy("Wrong type of argument: Expected a table stream type"),
    }
}

fn close(mut context: CommandContext) -> CrushResult<()> {
    let pipe = context.this.r#struct()?;
    pipe.set("read", Value::Empty);
    pipe.set("output", Value::Empty);
    Ok(())
}

fn write(mut context: CommandContext) -> CrushResult<()> {
    let pipe = context.this.r#struct()?;
    match mandate(pipe.get("output"), "Missing field")? {
        Value::TableOutputStream(output_stream) => {
            let mut stream = mandate(context.input.recv()?.stream()?, "Expected a stream")?;

            while let Ok(row) = stream.read() {
                output_stream.send(row)?;
            }
            context.output.send(Value::Empty)?;
            Ok(())
        }
        _ => argument_error_legacy("Expected an output stream")
    }
}
