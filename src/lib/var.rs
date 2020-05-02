use crate::lang::errors::{argument_error, mandate, CrushResult};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::scope::Scope;
use crate::lang::table::{ColumnType, Row};
use crate::lang::value::{Value, ValueType};
use std::collections::HashMap;

pub fn r#let(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments {
        context.env.declare(
            mandate(arg.argument_type, "Missing variable name")?.as_ref(),
            arg.value,
        )?;
    }
    Ok(())
}

pub fn set(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments {
        context.env.set(
            mandate(arg.argument_type, "Missing variable name")?.as_ref(),
            arg.value,
        )?;
    }
    Ok(())
}

pub fn unset(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments {
        if let Value::String(s) = &arg.value {
            if s.len() == 0 {
                return argument_error("Illegal variable name");
            } else {
                context.env.remove_str(s)?;
            }
        } else {
            return argument_error("Illegal variable name");
        }
    }
    Ok(())
}

pub fn r#use(context: ExecutionContext) -> CrushResult<()> {
    for arg in context.arguments.iter() {
        match (arg.argument_type.is_none(), &arg.value) {
            (true, Value::Scope(e)) => context.env.r#use(e),
            _ => return argument_error("Expected all arguments to be scopes"),
        }
    }
    Ok(())
}

pub fn env(context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("type", ValueType::String),
    ])?;

    let mut values: HashMap<String, ValueType> = HashMap::new();
    context.env.dump(&mut values)?;

    let mut keys = values.keys().collect::<Vec<&String>>();
    keys.sort();

    for k in keys {
        context.printer.handle_error(output.send(Row::new(vec![
            Value::String(k.clone()),
            Value::String(values[k].to_string()),
        ])));
    }

    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_lazy_namespace(
        "var",
        Box::new(move |ns| {
            ns.declare_command(
                "let", r#let, false,
                "name := value", "Declare a new variable", None)?;
            ns.declare_command(
                "set", set, false,
                "name = value", "Assign a new value to an already existing variable", None)?;
            ns.declare_command(
                "unset", unset, false,
                "scope name:string",
                "Removes a variable from the namespace",
                None)?;
            ns.declare_command(
                "env", env, false,
                "env", "Returns a table containing the current namespace",
                Some(r#"    The columns of the table are the name, and the type of the value."#))?;
            ns.declare_command(
                "use", r#use, false,
                "use scope:scope",
                "Puts the specified scope into the list of scopes to search in by default during scope lookups",
                Some(r#"    Example:

    use math
    sqrt 1.0"#))?;
            Ok(())
        }))?;
    Ok(())
}
