use crate::lang::{command::ExecutionContext, table::ColumnType, argument::Argument};
use crate::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, command::SimpleCommand, value_type::ValueType};
use crate::scope::Scope;
use crate::lib::parse_util::{single_argument_type, single_argument_list, two_arguments};

fn to(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.cast(single_argument_type(context.arguments)?)?)
}

fn of(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(context.input.recv()?.value_type()))
}

fn list(mut context: ExecutionContext) -> CrushResult<()> {
    let l = single_argument_type(context.arguments)?;
    context.output.send(Value::Type(ValueType::List(Box::new(l))))
}

fn dict(mut context: ExecutionContext) -> CrushResult<()> {
    two_arguments(&context.arguments)?;
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Type(key_type), Value::Type(value_type)) => {
            context.output.send(Value::Type(ValueType::Dict(Box::new(key_type), Box::new(value_type))))
        }
        _ => return argument_error("Expected two types as input")
    }
}

fn parse_column_types(mut arguments: Vec<Argument>) -> CrushResult<Vec<ColumnType>> {
    let mut types = Vec::new();

    for arg in arguments.drain(..) {
        if let Value::Type(t) = arg.value {
            types.push(ColumnType::new(arg.name, t));
        } else {
            return argument_error(format!("Expected all parameters to be types, found {}", arg.value.value_type().to_string()).as_str())
        }
    }
    Ok(types)
}

fn r#struct(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::Struct(parse_column_types(context.arguments)?)))
}

fn r#table(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::Table(parse_column_types(context.arguments)?)))
}

fn r#table_stream(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::TableStream(parse_column_types(context.arguments)?)))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("type")?;

    env.declare_str("to", Value::Command(SimpleCommand::new(to, true)))?;
    env.declare_str("of", Value::Command(SimpleCommand::new(of, false)))?;

    env.declare_str("list", Value::Command(SimpleCommand::new(list, false)))?;
    env.declare_str("dict", Value::Command(SimpleCommand::new(dict, false)))?;
    env.declare_str("struct", Value::Command(SimpleCommand::new(r#struct, false)))?;
    env.declare_str("table", Value::Command(SimpleCommand::new(table, false)))?;
    env.declare_str("table_stream", Value::Command(SimpleCommand::new(table_stream, false)))?;

    env.declare_str("integer", Value::Type(ValueType::Integer))?;
    env.declare_str("type", Value::Type(ValueType::Type))?;
    env.declare_str("text", Value::Type(ValueType::Text))?;
    env.declare_str("bool", Value::Type(ValueType::Bool))?;
    env.declare_str("closure", Value::Type(ValueType::Closure))?;
    env.declare_str("empty", Value::Type(ValueType::Empty))?;
    env.declare_str("field", Value::Type(ValueType::Field))?;
    env.declare_str("float", Value::Type(ValueType::Float))?;
    env.declare_str("duration", Value::Type(ValueType::Duration))?;
    env.declare_str("time", Value::Type(ValueType::Time))?;
    env.declare_str("command", Value::Type(ValueType::Command))?;
    env.declare_str("file", Value::Type(ValueType::File))?;
    env.declare_str("glob", Value::Type(ValueType::Glob))?;
    env.declare_str("regex", Value::Type(ValueType::Regex))?;
    env.declare_str("env", Value::Type(ValueType::Scope))?;
    env.declare_str("any", Value::Type(ValueType::Any))?;
    env.declare_str("binary", Value::Type(ValueType::Binary))?;
    env.declare_str("binary_stream", Value::Type(ValueType::BinaryStream))?;

    env.readonly();
    Ok(())
}
