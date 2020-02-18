use crate::lib::ExecutionContext;
use crate::errors::{CrushResult, argument_error};
use crate::data::{Value, Command, ValueType};
use crate::namepspace::Namespace;
use crate::lib::parse_util::single_argument_type;

fn to(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.cast(single_argument_type(context.arguments)?)?)
}

fn of(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(context.input.recv()?.value_type()))
}

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("type")?;

    env.declare_str("to", Value::Command(Command::new(to)))?;
    env.declare_str("of", Value::Command(Command::new(of)))?;

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
    env.declare_str("op", Value::Type(ValueType::Op))?;
    env.declare_str("env", Value::Type(ValueType::Env))?;
    env.declare_str("any", Value::Type(ValueType::Any))?;
    env.declare_str("binary", Value::Type(ValueType::Binary))?;
    /*
    Missing types:
    Stream(Vec<ColumnType>),
    Rows(Vec<ColumnType>),
    Row(Vec<ColumnType>),
    List(Box<ValueType>),
    Dict(Box<ValueType>, Box<ValueType>),
    */
    Ok(())
}
