use crate::lang::errors::{CrushResult, mandate};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::This;
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;
use crate::lang::argument::column_names;
use crate::lang::r#struct::Struct;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("new"), CrushCommand::command(
            new, false,
            "type:new [parent:type] name=value",
            "Create a new type",
            None));
        res
    };
}

fn new(context: ExecutionContext) -> CrushResult<()> {
    let mut names = column_names(&context.arguments);
    let arr: Vec<(Box<str>, Value)> =
        names.drain(..)
            .zip(context.arguments)
            .map(|(name, arg)| (name, arg.value))
            .collect::<Vec<(Box<str>, Value)>>();
    context.output.send(
        Value::Struct(Struct::new(arr)))
}
