use crate::builtins::types::column_types;
use crate::lang::any_str::AnyStr;
use crate::lang::command::Command;
use crate::lang::command::CrushCommand;
use crate::lang::command::OutputType::Known;
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, argument_error, command_error};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::streams;
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
        GetItem::declare_method(&mut res);
        Pipe::declare_method(&mut res);

        res
    })
}

pub fn close_value() -> &'static Value {
    static CELL: OnceLock<Value> = OnceLock::new();
    CELL.get_or_init(|| {
        Value::Command(<dyn CrushCommand>::command(
            close,
            false,
            &["global", "types", "pipe", "close"],
            "pipe:close",
            "Close the specified pipe",
            None::<AnyStr>,
            Known(ValueType::Empty),
            [],
        ))
    })
}

pub fn write_value() -> &'static Value {
    static CELL: OnceLock<Value> = OnceLock::new();
    CELL.get_or_init(|| {
        Value::Command(<dyn CrushCommand>::command(
            write,
            true,
            &["global", "types", "pipe", "write"],
            "pipe:write",
            "Write sink for this pipe",
            None::<AnyStr>,
            Known(ValueType::Empty),
            [],
        ))
    })
}

#[signature(
    types.table_input_stream.__call__,
    can_block = false,
    output = Known(ValueType::Type),
    short = "return the table_input_stream type with the specified column signature.",
    long = "You usually do this in order to create a pipe specialized to a specific column signature.",
    example = "$pipe := $($(table_input_stream value=$integer):pipe)",
)]
struct Call {
    #[description("the columns of the stream.")]
    #[named()]
    columns: OrderedStringMap<ValueType>,
}

fn __call__(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableInputStream(c) => {
            let cfg: Call = Call::parse(context.remove_arguments(), &context.global_state.printer())?;
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableInputStream(column_types(
                        &cfg.columns,
                    ))))
            } else if cfg.columns.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableInputStream(c)))
            } else {
                argument_error(
                    "Tried to set columns on a `table_input_stream` type that already has columns.",
                    &context.source,
                )
            }
        }
        _ => command_error("Invalid `this`, expected type `table_input_stream`."),
    }
}

#[signature(
    types.table_input_stream.__getitem__,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Returns the specified row of the table stream as a struct.",
    example = "$(files)[4]"
)]
struct GetItem {
    index: i128,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    let o = context.this.table_input_stream()?;
    context
        .output
        .send(Value::Struct(o.get(cfg.index)?.into_struct(o.types())))
}

#[signature(
    types.table_input_stream.pipe,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Returns a pipe consisting of a read end and a write end.",
    long = "Each row of data in the pipe must have the columns specified by this table_input_stream specialization.",
    long = "A pipe is usually created by specializing table_input_stream e.g. like",
    long = "",
    long = "    $pipe := $($(table_input_stream value=$integer):pipe)",
    long = "",
    long = "The pipe object has three methods:",
    long = "",
    long = " * `pipe:write` write sink for this pipe. Put this method at the end of a pipeline that produces data for the pipe.",
    long = " * `pipe:read` read source for this pipe. Put this method at the start of a pipeline that consumes data from the pipe.",
    long = " * `pipe:close` call this method once all readers and writers have been created in order to close the pipe.",
    long = "",
    long = "A pipe object can have arbitrarily many write jobs producing data into the pipe.",
    long = "Each writer simply pipes rows into the pipe:write method.",
    long = "",
    long = "A pipe object can have arbitrarily many read jobs consuming data from the pipe.",
    long = "Each reader simply consumes rows from the pipe:read method.",
    long = "",
    long = "Each row written to the pipe will be consumed by exactly one reader job.",
    long = "",
    long = "In order for the consumer jobs to finish, all the writer jobs must end *and* the",
    long = "pipe:close method must be called. Once this has happened and all the rows have been",
    long = "processed, the consumer job(s) will finish.",
    long = "",
    long = "Note that the pipe:close method does not interrupt currently existing read or write",
    long = "jobs, but it does prevent new read and write jobs from being started.",
    example = "# Create a pipe",
    example = "$pipe := $($(table_input_stream value=$integer):pipe)",
    example = "# Create a job that writes 100_000 integers to the pipe and put this job in the background",
    example = "seq 100_000 | pipe:write &",
    example = "# Create a second job that reads from the pipe and sums all the integers and put this job in the background",
    example = "$sum_job_handle := $(pipe:read | sum &)",
    example = "# Close the pipe so that the second job can finish",
    example = "pipe:close",
    example = "# Put the sum job in the foreground",
    example = "fg $sum_job_handle",
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
        _ => command_error("Wrong type of argument: Expected a table stream type."),
    }
}

/// Close a pipe.
/// This is done be clearing the `read` and `output` fields.
fn close(mut context: CommandContext) -> CrushResult<()> {
    let pipe = context.this.r#struct()?;
    pipe.set("read", Value::Empty);
    pipe.set("output", Value::Empty);
    Ok(())
}

fn write(mut context: CommandContext) -> CrushResult<()> {
    let pipe = context.this.r#struct()?;
    match pipe.get("output") {
        Some(Value::TableOutputStream(output_stream)) => {
            let mut stream = context.input.recv()?.stream()?.ok_or("Expected a stream")?;

            while let Ok(row) = stream.read() {
                output_stream.send(row)?;
            }
            context.output.send(Value::Empty)?;
            Ok(())
        }
        _ => command_error("Expected an output stream."),
    }
}
