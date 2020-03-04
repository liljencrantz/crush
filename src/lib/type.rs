use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::lang::{Value, SimpleCommand, ValueType};
use crate::scope::Scope;
use crate::lib::parse_util::{single_argument_type, single_argument_list};

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

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("type")?;

    env.declare_str("to", Value::Command(SimpleCommand::new(to, false)))?;
    env.declare_str("of", Value::Command(SimpleCommand::new(of, false)))?;

    env.declare_str("list", Value::Command(SimpleCommand::new(list, false)))?;

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
    env.declare_str("env", Value::Type(ValueType::Env))?;
    env.declare_str("any", Value::Type(ValueType::Any))?;
    env.declare_str("binary", Value::Type(ValueType::Binary))?;
    env.declare_str("binary_stream", Value::Type(ValueType::BinaryStream))?;
    /*
    Missing types:
    Stream(Vec<ColumnType>),
    Rows(Vec<ColumnType>),
    Row(Vec<ColumnType>),
    Dict(Box<ValueType>, Box<ValueType>),
    */
    env.readonly();
    Ok(())
}
