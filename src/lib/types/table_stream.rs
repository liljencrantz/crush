use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{This};
use crate::lang::value::ValueType;
use crate::lang::{execution_context::CommandContext, value::Value};
use crate::lib::types::column_types;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::ordered_string_map::OrderedStringMap;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "table_stream"];
        Call::declare_method(&mut res, &path);
        GetItem::declare_method(&mut res, &path);
        res
    };
}

#[signature(
__call__,
can_block = false,
output = Known(ValueType::Type),
short = "return the table_stream type with the specified column signature.",
)]
struct Call {
    #[description("the columns of the stream.")]
    #[named()]
    columns: OrderedStringMap<ValueType>,
}

fn __call__(context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableStream(c) => {
            let cfg: Call = Call::parse(context.arguments, &context.printer)?;
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableStream(column_types(&cfg.columns))))
            } else if cfg.columns.is_empty() {
                context.output.send(Value::Type(ValueType::TableStream(c)))
            } else {
                argument_error(
                    "Tried to set columns on a table_stream type that already has columns",
                )
            }
        }
        _ => argument_error("Invalid this, expected type table_stream"),
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

fn __getitem__(context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.arguments, &context.printer)?;
    let o = context.this.table_stream()?;
    context.output.send(Value::Struct(o.get(cfg.index)?.into_struct(o.types())))
}
