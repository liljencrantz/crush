use crate::lang::command::Command;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, This};
use crate::lang::value::ValueType;
use crate::lang::{execution_context::ExecutionContext, value::Value};
use crate::lib::types::parse_column_types;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "table_stream", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        res.declare(
            full("__call_type__"),
            call_type,
            false,
            "table_stream column_name=type:type...",
            "Return the table_stream type with the specified column signature",
            None,
            Known(ValueType::Type),
        );
        res.declare(
            full("__getitem__"),
            getitem,
            false,
            "table_stream[idx:integer]",
            "Returns the specified row of the table stream",
            None,
            Unknown,
        );
        res
    };
}

fn call_type(context: ExecutionContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableStream(c) => {
            if c.is_empty() {
                context
                    .output
                    .send(Value::Type(ValueType::TableStream(parse_column_types(
                        context.arguments,
                    )?)))
            } else if context.arguments.is_empty() {
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

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    let o = context.this.table_stream()?;
    context.arguments.check_len(1)?;
    let idx = context.arguments.integer(0)?;
    context
        .output
        .send(Value::Struct(o.get(idx)?.into_struct(o.types())))
}
