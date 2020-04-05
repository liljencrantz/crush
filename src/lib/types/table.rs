use crate::lang::errors::{CrushResult, mandate, CrushError};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::command::CrushCommand;
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;
use crate::lib::types::parse_column_types;
use crate::lang::execution_context::{This, ArgumentVector};
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "table", name]
}

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.declare(
            full("__call_type__"), call_type, false,
            "table column_name=type:type...",
            "Return the table type with the specified column signature",
            None);
        res.declare(full("len"), len, false, "table:len", "The number of rows in the table", None);
        res.declare(
            full("__getitem__"),
            getitem, false,
            "table[idx:integer]", "Returns the specified row of the table as a struct", None);
        res
    };
}

fn call_type(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::Table(parse_column_types(context.arguments)?)))
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    let table = context.this.table()?;
    context.output.send(Value::Integer(table.rows().len() as i128))
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    let o = context.this.table()?;
    context.arguments.check_len(1)?;
    let idx = context.arguments.integer(0)?;
    context.output.send(Value::Struct(
        mandate(o.rows().get(idx as usize), "Index out of range")?
            .clone()
            .into_struct(o.types())))
}
