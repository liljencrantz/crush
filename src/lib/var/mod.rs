use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, mandate};
use crate::lang::{value::Value, command::CrushCommand};
use crate::lang::execution_context::ExecutionContext;

mod env;
mod r#use;

pub fn r#let(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments {
        context.env.declare(mandate(arg.argument_type, "Missing variable name")?.as_ref(), arg.value)?;
    }
    Ok(())
}

pub fn set(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments {
        context.env.set(mandate(arg.argument_type, "Missing variable name")?.as_ref(), arg.value)?;
    }
    Ok(())
}

pub fn unset(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments {
        if let Value::String(s) = &arg.value {
            if s.len() == 0 {
                return argument_error("Illegal variable name");
            } else {
                context.env.remove_str(s);
            }
        } else {
            return argument_error("Illegal variable name");
        }
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("var")?;
    env.declare("let", Value::Command(CrushCommand::command_undocumented(r#let, false)))?;
    env.declare("set", Value::Command(CrushCommand::command_undocumented(set, false)))?;
    env.declare("unset", Value::Command(CrushCommand::command_undocumented(unset, false)))?;
    env.declare("env", Value::Command(CrushCommand::command_undocumented(env::perform, false)))?;
    env.declare("use", Value::Command(CrushCommand::command_undocumented(r#use::perform, false)))?;
    env.readonly();
    Ok(())
}
