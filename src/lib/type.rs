use crate::lang::{command::ExecutionContext, table::ColumnType, argument::Argument};
use crate::lang::errors::{CrushResult, argument_error, mandate};
use crate::lang::{value::Value, value::ValueType};
use crate::lang::scope::Scope;
use crate::lang::argument::column_names;
use crate::lang::command::{CrushCommand, ArgumentVector};

fn to(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.cast(context.arguments.r#type(0)?)?)
}

pub fn r#type(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(mandate(context.this, "Missing this value")?.value_type()))
}

fn list(mut context: ExecutionContext) -> CrushResult<()> {
    let l = context.arguments.r#type(0)?;
    context.output.send(Value::Type(ValueType::List(Box::new(l))))
}

fn dict(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let key_type = context.arguments.r#type(0)?;
    let value_type = context.arguments.r#type(1)?;
    context.output.send(Value::Type(ValueType::Dict(Box::new(key_type), Box::new(value_type))))
}

fn parse_column_types(mut arguments: Vec<Argument>) -> CrushResult<Vec<ColumnType>> {
    let mut types = Vec::new();
    let names = column_names(&arguments);

    for (idx, arg) in arguments.drain(..).enumerate() {
        if let Value::Type(t) = arg.value {
            types.push(ColumnType::new(names[idx].as_ref(), t));
        } else {
            return argument_error(format!("Expected all parameters to be types, found {}", arg.value.value_type().to_string()).as_str())
        }
    }
    Ok(types)
}

fn r#struct(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::Struct(parse_column_types(context.arguments)?)))
}

fn r#table(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::Table(parse_column_types(context.arguments)?)))
}

fn r#table_stream(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::TableStream(parse_column_types(context.arguments)?)))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("type")?;

    env.declare("to", Value::Command(CrushCommand::command(to, true)))?;
//    env.declare("of", Value::Command(CrushCommand::command(of, false)))?;

    env.declare("list", Value::Command(CrushCommand::command(list, false)))?;
    env.declare("dict", Value::Command(CrushCommand::command(dict, false)))?;
    env.declare("struct", Value::Command(CrushCommand::command(r#struct, false)))?;
    env.declare("table", Value::Command(CrushCommand::command(table, false)))?;
    env.declare("table_stream", Value::Command(CrushCommand::command(table_stream, false)))?;

    env.declare("integer", Value::Type(ValueType::Integer))?;
    env.declare("type", Value::Type(ValueType::Type))?;
    env.declare("string", Value::Type(ValueType::String))?;
    env.declare("bool", Value::Type(ValueType::Bool))?;
    env.declare("empty", Value::Type(ValueType::Empty))?;
    env.declare("field", Value::Type(ValueType::Field))?;
    env.declare("float", Value::Type(ValueType::Float))?;
    env.declare("duration", Value::Type(ValueType::Duration))?;
    env.declare("time", Value::Type(ValueType::Time))?;
    env.declare("command", Value::Type(ValueType::Command))?;
    env.declare("file", Value::Type(ValueType::File))?;
    env.declare("glob", Value::Type(ValueType::Glob))?;
    env.declare("regex", Value::Type(ValueType::Regex))?;
    env.declare("env", Value::Type(ValueType::Scope))?;
    env.declare("any", Value::Type(ValueType::Any))?;
    env.declare("binary", Value::Type(ValueType::Binary))?;
    env.declare("binary_stream", Value::Type(ValueType::BinaryStream))?;

    env.readonly();
    Ok(())
}
