use crate::commands::CompileContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::{Command, Value};
use crate::env::Env;
use std::cmp::Ordering;

fn gt(mut context: CompileContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }
    let l = context.arguments.remove(0).value;
    let r = context.arguments.remove(0).value;

    match l.partial_cmp(&r) {
        Some(ordering) => {
            context.output.send(Value::Bool(ordering == Ordering::Greater))?;
        }
        None => return argument_error("Uncomparable values"),
    }
    Ok(())
}

pub fn declare(root: &Env) -> CrushResult<()> {
    let list = root.create_namespace("comp")?;
    list.declare_str("gt", Value::Command(Command::new(gt)))?;
    Ok(())
}
