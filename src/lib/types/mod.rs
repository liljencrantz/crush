use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, mandate};
use crate::lang::{value::Value, r#struct::Struct};
use crate::lang::command::CrushCommand;
use crate::lang::execution_context::{ExecutionContext, This};
use crate::lang::argument::{column_names, Argument};
use crate::lang::execution_context::ArgumentVector;
use crate::lang::value::ValueType;
use crate::lang::table::ColumnType;

pub mod table;
pub mod table_stream;
pub mod list;
pub mod dict;
pub mod re;
pub mod glob;
pub mod string;
pub mod file;
pub mod integer;
pub mod float;
pub mod duration;
pub mod time;

fn materialize(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize())
}

fn struct_of(context: ExecutionContext) -> CrushResult<()> {
    let mut names = column_names(&context.arguments);

    let arr: Vec<(Box<str>, Value)> =
        names.drain(..)
            .zip(context.arguments)
            .map(|(name, arg)| (name, arg.value))
            .collect::<Vec<(Box<str>, Value)>>();
    context.output.send(
        Value::Struct(Struct::new(arr)))
}

pub fn setattr(mut context: ExecutionContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let name = context.arguments.string(0)?;
    let value = context.arguments.value(1)?;
    this.set(&name, value);
    Ok(())
}

pub fn parse_column_types(mut arguments: Vec<Argument>) -> CrushResult<Vec<ColumnType>> {
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

pub fn struct_call_type(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(ValueType::Struct(parse_column_types(context.arguments)?)))
}

pub fn r#as(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.arguments.value(0)?.cast(context.arguments.r#type(1)?)?)
}

pub fn r#type(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Type(mandate(context.this, "Missing this value")?.value_type()))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("types")?;
    root.r#use(&env);

    env.declare("struct_of", Value::Command(CrushCommand::command_undocumented(struct_of, false)))?;
    env.declare("materialize", Value::Command(CrushCommand::command_undocumented(materialize, true)))?;

    env.declare("file", Value::Type(ValueType::File))?;
    env.declare("type", Value::Type(ValueType::Type))?;
    env.declare("any", Value::Type(ValueType::Any))?;
    env.declare("bool", Value::Type(ValueType::Bool))?;
    env.declare("command", Value::Type(ValueType::Command))?;
    env.declare("scope", Value::Type(ValueType::Scope))?;
    env.declare("binary", Value::Type(ValueType::Binary))?;
    env.declare("binary_stream", Value::Type(ValueType::BinaryStream))?;
    env.declare("field", Value::Type(ValueType::Field))?;
    env.declare("empty", Value::Type(ValueType::Empty))?;
    env.declare("float", Value::Type(ValueType::Float))?;
    env.declare("integer", Value::Type(ValueType::Integer))?;
    env.declare("list", Value::Type(ValueType::List(Box::from(ValueType::Empty))))?;
    env.declare("string", Value::Type(ValueType::String))?;
    env.declare("glob", Value::Type(ValueType::Glob))?;
    env.declare("re", Value::Type(ValueType::Regex))?;
    env.declare("duration", Value::Type(ValueType::Duration))?;
    env.declare("time", Value::Type(ValueType::Time))?;
    env.declare("dict", Value::Type(ValueType::Dict(Box::from(ValueType::Empty), Box::from(ValueType::Empty))))?;

    env.declare("table", Value::Type(ValueType::Table(vec![])))?;
    env.declare("table_stream", Value::Type(ValueType::TableStream(vec![])))?;
    env.declare("struct", Value::Type(ValueType::Struct(vec![])))?;

    env.readonly();

    Ok(())
}
