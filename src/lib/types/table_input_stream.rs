use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error_legacy, CrushResult, mandate};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use crate::lib::types::column_types;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::streams;
use crate::lang::data::r#struct::Struct;
use crate::lang::command::CrushCommand;
use crate::lang::state::this::This;

lazy_static! {
    static ref CLOSE: Value =
        Value::Command(<dyn CrushCommand>::command(
            close, false,
            &["global", "types", "pipe", "close"],
            "pipe:close",
            "Close the specified pipe",
            None,
            Known(ValueType::Empty),
            [],
        ));

        static ref WRITE: Value =
        Value::Command(<dyn CrushCommand>::command(
            write, true,
            &["global", "types", "pipe", "write"],
            "pipe:write",
            "Write sink for this pipe",
            None,
            Known(ValueType::Empty),
            [],
        ));

    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "table_input_stream"];
        Call::declare_method(&mut res, &path);
        GetItem::declare_method(&mut res, &path);
        Pipe::declare_method(&mut res, &path);
        res
    };
}

#[signature(
__call__,
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
__getitem__,
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
pipe,
can_block = false,
output = Known(ValueType::Struct),
short = "Returns a struct containing a read end and a write end of a pipe of the specified type",
example = "$pipe := ((table_input_stream value=$integer):pipe)\n    $_1 := (seq 100_000 | $pipe:write | bg)\n    $sum_job_id := ($pipe:read | sum | bg)\n    $pipe:close\n    $sum_job_id | fg"
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
                    ("write", WRITE.clone()),
                    ("close", CLOSE.clone()),
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
