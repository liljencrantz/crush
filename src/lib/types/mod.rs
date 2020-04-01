use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, r#struct::Struct};
use crate::lang::command::CrushCommand;
use crate::lang::execution_context::{ExecutionContext, This};
use crate::lang::argument::{column_names, Argument};
use crate::lang::execution_context::ArgumentVector;
use crate::lang::value::ValueType;
use crate::lang::table::ColumnType;
use crate::lang::stream::black_hole;

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
pub mod binary;
pub mod scope;

fn materialize(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize())
}

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    let parent = context.this.clone().r#struct()?;
    let res = Struct::new(vec![], Some(parent));
    let init = res.get("__init__");
    let o = context.output;
    context.output = black_hole();
    context.this = Some(Value::Struct(res.clone()));
    match init {
        Some(Value::Command(c)) => { c.invoke(context)?; }
        _ => {}
    };
    o.send(Value::Struct(res))
}

fn data(context: ExecutionContext) -> CrushResult<()> {
    let mut names = column_names(&context.arguments);
    let arr: Vec<(Box<str>, Value)> =
        names.drain(..)
            .zip(context.arguments)
            .map(|(name, arg)| (name, arg.value))
            .collect::<Vec<(Box<str>, Value)>>();
    context.output.send(
        Value::Struct(Struct::new(arr, None)))
}

fn class(mut context: ExecutionContext) -> CrushResult<()> {
    let mut parent = crate::lang::r#struct::ROOT.clone();

    if context.arguments.len() == 1 {
        parent = context.arguments.r#struct(0)?;
    }

    let res = Struct::new(
        vec![
            (Box::from("new"), Value::Command(CrushCommand::command_undocumented(new, true))),
        ],
        Some(parent),
    );

    context.output.send(Value::Struct(res))
}

pub fn parse_column_types(mut arguments: Vec<Argument>) -> CrushResult<Vec<ColumnType>> {
    let mut types = Vec::new();
    let names = column_names(&arguments);

    for (idx, arg) in arguments.drain(..).enumerate() {
        if let Value::Type(t) = arg.value {
            types.push(ColumnType::new(names[idx].as_ref(), t));
        } else {
            return argument_error(format!("Expected all parameters to be types, found {}", arg.value.value_type().to_string()).as_str());
        }
    }
    Ok(types)
}

pub fn r#as(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.arguments.value(0)?.cast(context.arguments.r#type(1)?)?)
}

pub fn r#typeof(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(Value::Type(context.arguments.value(0)?.value_type()))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("types")?;
    root.r#use(&env);

    env.declare("data", Value::Command(CrushCommand::command(
        data, false,
        "data <name>=value:any...",
        "Construct a struct with the specified members",
        None)))?;

    env.declare("class", Value::Command(CrushCommand::command(
        class, false,
        "class [parent:type]",
        "Create an empty new class",
        Some(r#"    Example:

    Point := (class)
    Point:__init__ = {
        |x:float y:float|
        this:x = x
        this:y = y
    }

    Point:len = {
        ||
        math.sqrt this:x*this:x + this:y*this:y
    }

    Point:__add__ = {
        |other|
        Point:new x=(this:x + other:x) y=(this:y + other:y)
    }

    p := (Point:new x=1.0 y=2.0)
    p:len"#))))?;
    env.declare("materialize", Value::Command(CrushCommand::command(
        materialize, true,
        "materialize",
        "Recursively convert all streams in input to materialized version",
        example!("ls | materialize"))))?;

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
    env.declare("dict", Value::Type(ValueType::Dict(
        Box::from(ValueType::Empty),
        Box::from(ValueType::Empty))))?;

    env.declare("table", Value::Type(ValueType::Table(vec![])))?;
    env.declare("table_stream", Value::Type(ValueType::TableStream(vec![])))?;
    env.declare("struct", Value::Type(ValueType::Struct))?;

    env.readonly();

    Ok(())
}
