use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::command::CrushCommand;
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;
use crate::lib::types::parse_column_types;
use crate::lang::execution_context::This;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("__call_type__"), CrushCommand::command_undocumented(call_type, false));
        res.insert(Box::from("len"), CrushCommand::command_undocumented(len, false));
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
