use crate::lang::errors::{CrushResult, mandate};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::command::CrushCommand;
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;
use crate::lib::types::parse_column_types;
use crate::lang::execution_context::{This, ArgumentVector};

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("__call_type__"), CrushCommand::command_undocumented(call_type, false));
        res.insert(Box::from("len"), CrushCommand::command_undocumented(len, false));
        res.insert(Box::from("__getitem__"), CrushCommand::command(
            getitem, false,
            "table[idx:integer]", "Returns the specified row of the table", None));
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
