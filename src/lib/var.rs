use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::errors::{argument_error, mandate, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::scope::Scope;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::value::{Value, ValueType};
use ordered_map::OrderedMap;

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
                return argument_error("Illegal variable name");
            } else {
                context.scope.remove_str(s)?;
            }
        } else {
            return argument_error("Illegal variable name");
        }
    }
    context.output.send(Value::Empty())
}

pub fn r#use(context: CommandContext) -> CrushResult<()> {
    for arg in context.arguments.iter() {
        match (arg.argument_type.is_none(), &arg.value) {
            (true, Value::Scope(e)) => context.scope.r#use(e),
            _ => return argument_error("Expected all arguments to be scopes"),
        }
    }
    context.output.send(Value::Empty())
}

pub fn env(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("type", ValueType::String),
    ])?;

    let mut values: OrderedMap<String, ValueType> = OrderedMap::new();
    context.scope.dump(&mut values)?;

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
    root.create_namespace(
        "var",
        Box::new(move |ns| {
            ns.declare_command(
                "let", r#let, false,
                "name := value", "Declare a new variable", None, Known(ValueType::Empty))?;
            ns.declare_command(
                "set", set, false,
                "name = value", "Assign a new value to an already existing variable", None, Known(ValueType::Empty))?;
            ns.declare_command(
                "unset", unset, false,
                "scope name:string",
                "Removes a variable from the namespace",
                None, Known(ValueType::Empty))?;
            ns.declare_command(
                "env", env, false,
                "env", "Returns a table containing the current namespace",
                Some(r#"    The columns of the table are the name, and the type of the value."#), Unknown)?;
            ns.declare_command(
                "use", r#use, false,
                "use scope:scope",
                "Puts the specified scope into the list of scopes to search in by default during scope lookups",
                Some(r#"    Example:

    use math
    sqrt 1.0"#), Known(ValueType::Empty))?;
            Ok(())
        }))?;
    Ok(())
}
