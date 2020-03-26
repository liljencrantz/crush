use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::{value::Value, command::ExecutionContext};
use regex::Regex;
use std::error::Error;
use crate::lang::command::{CrushCommand, ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;
use crate::util::glob::Glob;
use crate::lib::binary_op;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("__add__"), CrushCommand::command(add, false));
        res.insert(Box::from("__sub__"), CrushCommand::command(sub, false));
        res.insert(Box::from("__mul__"), CrushCommand::command(mul, false));
        res.insert(Box::from("__div__"), CrushCommand::command(div, false));
        res.insert(Box::from("__neg__"), CrushCommand::command(neg, false));
        res
    };
}

binary_op!(add, integer, Integer, Integer, |a, b| a+b, Float, Float, |a, b| a as f64+b);
binary_op!(sub, integer, Integer, Integer, |a, b| a-b, Float, Float, |a, b| a as f64-b);
binary_op!(mul, integer, Integer, Integer, |a, b| a*b, Float, Float, |a, b| a as f64*b);
binary_op!(div, integer, Integer, Integer, |a, b| a/b, Float, Float, |a, b| a as f64/b);

fn neg(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(-context.this.float()?))
}
