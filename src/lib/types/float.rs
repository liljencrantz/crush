use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("__add__"), CrushCommand::command_undocumented(add, false));
        res.insert(Box::from("__sub__"), CrushCommand::command_undocumented(sub, false));
        res.insert(Box::from("__mul__"), CrushCommand::command_undocumented(mul, false));
        res.insert(Box::from("__div__"), CrushCommand::command_undocumented(div, false));
        res.insert(Box::from("__neg__"), CrushCommand::command_undocumented(neg, false));
        res
    };
}

binary_op!(add, float, Integer, Float, |a, b| a+(b as f64), Float, Float, |a, b| a+b);
binary_op!(sub, float, Integer, Float, |a, b| a-(b as f64), Float, Float, |a, b| a-b);
binary_op!(mul, float, Integer, Float, |a, b| a*(b as f64), Float, Float, |a, b| a*b);
binary_op!(div, float, Integer, Float, |a, b| a/(b as f64), Float, Float, |a, b| a/b);

fn neg(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(-context.this.float()?))
}
