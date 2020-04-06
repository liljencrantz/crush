use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, mandate};
use crate::lang::value::Value;
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
    env.declare_command(
        "let", r#let, false,
        "name := value", "Declare a new variable", None)?;
    env.declare_command(
        "set", set, false,
        "name = value", "Assign a new value to an already existing variable", None)?;
    env.declare_command(
        "unset", unset, false,
        "scope name:string",
        "Removes a variable from the namespace",
        None)?;
    env.declare_command(
        "env", env::perform, false,
        "env", "Returns a table containing the current namespace",
        Some(r#"    The columns of the table are the name, and the type of the value."#))?;
    env.declare_command(
        "use", r#use::perform, false,
        "use scope:scope",
        "Puts the specified scope into the list of scopes to search in by default during scope lookups",
        Some(r#"    Example:

    use math
    sqrt 1.0"#))?;
    env.readonly();
    Ok(())
}
