use crate::lang::errors::{CrushResult};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::util::glob::Glob;
use crate::lang::command::CrushCommand;
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "glob", name]
}

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.declare(full("match"),
            r#match, false,
            "glob:match input:string", "True if the input matches the pattern", None);
        res.declare(full("not_match"),
            not_match, false,
            "glob:not_match input:string", "True if the input does not match the pattern", None);
        res.declare(full("new"),
            new, false,
            "glob:new pattern:string", "Return a new glob", None);
        res
    };
}

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    let def = context.arguments.string(0)?;
    context.output.send(Value::Glob(Glob::new(&def)))
}

fn r#match(mut context: ExecutionContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(g.matches(&needle)))
}

fn not_match(mut context: ExecutionContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(!g.matches(&needle)))
}
