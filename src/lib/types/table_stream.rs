use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::command::CrushCommand;
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;
use crate::lib::types::parse_column_types;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("__call_type__"), CrushCommand::command(call_type, false));
        res
    };
}

fn call_type(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::TableStream(parse_column_types(context.arguments)?)))
}
