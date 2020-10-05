use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::errors::{argument_error_legacy, mandate, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::scope::Scope;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::value::{Value, ValueType};

pub fn r#let(context: CommandContext) -> CrushResult<()> {
    for arg in context.arguments {
        context.scope.declare(
            mandate(arg.argument_type, "Missing variable name")?.as_ref(),
            arg.value,
        )?;
    }
    context.output.send(Value::Empty())
}

pub fn set(context: CommandContext) -> CrushResult<()> {
    for arg in context.arguments {
        context.scope.set(
            mandate(arg.argument_type, "Missing variable name")?.as_ref(),
            arg.value,
        )?;
    }
    context.output.send(Value::Empty())
}

pub fn unset(context: CommandContext) -> CrushResult<()> {
    for arg in context.arguments {
        if let Value::String(s) = &arg.value {
            if s.len() == 0 {
                return argument_error_legacy("Illegal variable name");
            } else {
                context.scope.remove_str(s)?;
            }
        } else {
            return argument_error_legacy("Illegal variable name");
        }
    }
    context.output.send(Value::Empty())
}

pub fn r#use(context: CommandContext) -> CrushResult<()> {
    for arg in context.arguments.iter() {
        match (arg.argument_type.is_none(), &arg.value) {
            (true, Value::Scope(e)) => context.scope.r#use(e),
            _ => return argument_error_legacy("Expected all arguments to be scopes"),
        }
    }
    context.output.send(Value::Empty())
}

pub fn env(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("type", ValueType::String),
    ])?;

    let values = context.scope.dump()?;

    let mut keys = values.keys().collect::<Vec<&String>>();
    keys.sort();

    for k in keys {
        context.global_state.printer().handle_error(output.send(Row::new(vec![
            Value::String(k.clone()),
            Value::String(values[k].to_string()),
        ])));
    }

    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "var",
        "Variable related commands",
        Box::new(move |ns| {
            ns.declare_command(
                "let", r#let, false,
                "name := value",
                "Declare a new variable",
                None,
                Known(ValueType::Empty),
                vec![],
            )?;
            ns.declare_command(
                "set", set, false,
                "name = value",
                "Assign a new value to an already existing variable",
                None,
                Known(ValueType::Empty),
                vec![],
            )?;
            ns.declare_command(
                "unset", unset, false,
                "scope name:string",
                "Removes a variable from the namespace",
                None,
                Known(ValueType::Empty),
                vec![],
            )?;
            ns.declare_command(
                "env", env, false,
                "env", "Returns a table containing the current namespace",
                Some(r#"    The columns of the table are the name, and the type of the value."#),
                Unknown,
                vec![],
            )?;
            ns.declare_command(
                "use", r#use, false,
                "use scope:scope",
                "Puts the specified scope into the list of scopes to search in by default during scope lookups",
                Some(r#"    Example:

    use math
    sqrt 1.0"#),
                Known(ValueType::Empty),
                vec![],
            )?;
            Ok(())
        }))?;
    Ok(())
}
