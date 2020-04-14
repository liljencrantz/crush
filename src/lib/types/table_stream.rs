use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::command::CrushCommand;
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;
use crate::lib::types::parse_column_types;
use crate::lang::execution_context::{This, ArgumentVector};
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "table_stream", name]
}

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.declare(
            full("__call_type__"), call_type, false,
            "table_stream column_name=type:type...",
            "Return the table_stream type with the specified column signature",
            None);
        res.declare(
            full("__getitem__"),
            getitem, false,
            "table_stream[idx:integer]", "Returns the specified row of the table stream",
            None);
        res
    };
}

fn call_type(context: ExecutionContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::TableStream(c) => {
            if c.is_empty() {
                context.output.send(Value::Type(ValueType::TableStream(parse_column_types(context.arguments)?)))
            } else if context.arguments.is_empty() {
                context.output.send(Value::Type(ValueType::TableStream(c)))
            } else {
                argument_error("Tried to set columns on a table_stream type that already has columns")
            }
        },
        _ => argument_error("Invalid this, expected type table_stream")
    }
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    let o = context.this.table_stream()?;
    context.arguments.check_len(1)?;
    let idx = context.arguments.integer(0)?;
    context.output.send(
        Value::Struct(o.get(idx)?.into_struct(o.types())))
}
