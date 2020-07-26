use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, argument_error, mandate};
use crate::lang::{value::Value, r#struct::Struct};
use crate::lang::command::CrushCommand;
use crate::lang::execution_context::{ExecutionContext, This};
use crate::lang::argument::{column_names, Argument};
use crate::lang::execution_context::ArgumentVector;
use crate::lang::value::ValueType;
use crate::lang::table::ColumnType;
use crate::lang::stream::black_hole;
use crate::lang::command::OutputType::{Known, Unknown};

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
    let o = context.output;

    // Call constructor if one exists
    if let Some(Value::Command(c)) = res.get("__init__") {
        context.output = black_hole();
        context.this = Some(Value::Struct(res.clone()));
        c.invoke(context)?;
    }
    o.send(Value::Struct(res))
}

fn data(context: ExecutionContext) -> CrushResult<()> {
    let mut names = column_names(&context.arguments);
    let arr: Vec<(String, Value)> =
        names.drain(..)
            .zip(context.arguments)
            .map(|(name, arg)| (name, arg.value))
            .collect::<Vec<(String, Value)>>();
    context.output.send(
        Value::Struct(Struct::new(arr, None)))
}

fn class(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len_range(0, 1)?;
    let parent = if !context.arguments.is_empty() {
        context.arguments.r#struct(0)?
    } else {
        context.env.root_object()
    };

    let res = Struct::new(vec![], Some(parent));

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

pub fn convert(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.send(context.arguments.value(0)?.convert(context.arguments.r#type(1)?)?)
}

pub fn r#typeof(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(Value::Type(context.arguments.value(0)?.value_type()))
}

fn class_set(mut context: ExecutionContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let value = context.arguments.value(1)?;
    let name = context.arguments.string(0)?;
    this.set(&name, value);
    context.output.send(Value::Empty())
}

fn class_get(mut context: ExecutionContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let name = context.arguments.string(0)?;
    context.output.send(mandate(this.get(&name), format!("Unknown field {}", name).as_str())?)
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_lazy_namespace(
        "types",
        Box::new(move |env| {
            let root =
                Struct::new(vec![
                    ("__setattr__".to_string(), Value::Command(CrushCommand::command(
                        class_set, false,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "__setattr__".to_string()],
                        "root:__setitem__ name:string value:any",
                        "Modify the specified field to hold the specified value",
                        None, Known(ValueType::Empty)))),
                    ("__getitem__".to_string(), Value::Command(CrushCommand::command(
                        class_get, false,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "__getitem__".to_string()],
                        "root:__getitem__ name:string",
                        "Return the value of the specified field",
                        None, Unknown))),
                    ("__setitem__".to_string(), Value::Command(CrushCommand::command(
                        class_get, false,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "__setitem__".to_string()],
                        "root:__setitem__ name:string value:any",
                        "Modify the specified field to hold the specified value",
                        None, Unknown))),
                    ("new".to_string(), Value::Command(CrushCommand::command(
                        new, true,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "new".to_string()],
                        "root:new @unnamed @@named",
                        "Create a new instance of the specified type",
                        None, Known(ValueType::Struct)))),
                ], None);

            env.declare("root", Value::Struct(root))?;


            env.declare_command("class_get", class_get, false,
                                "root:__getitem__ name:string",
                                "Return the value of the specified field",
                                None, Unknown)?;

            env.declare_command("data", data, false,
                                "data <name>=value:any...",
                                "Construct a struct with the specified members",
                                None, Known(ValueType::Struct))?;

            env.declare_command("convert", convert, false,
                                "convert value:any type:type",
                                "Convert the vale to the specified type",
                                None, Unknown)?;

            env.declare_command("typeof", r#typeof, false,
                                "typeof value:any",
                                "Return the type of the specified value",
                                None, Known(ValueType::Type))?;

            env.declare_command(
                "class", class, false,
                "class [parent:type]",
                "Create an empty new class",
                Some(r#"    Example:

    Point := class

    Point:__init__ = {
        |x:float y:float|
        this:x = x
        this:y = y
    }

    Point:len = {
        ||
        math:sqrt this:x*this:x + this:y*this:y
    }

    Point:__add__ = {
        |other|
        Point:new x=this:x+other:x y=this:y+other:y
    }

    p := (Point:new x=1.0 y=2.0)
    p:len"#), Known(ValueType::Type))?;
            env.declare_command(
                "materialize", materialize, true,
                "materialize",
                "Recursively convert all streams in input to materialized form",
                Some(r#"    The purpose of materializing a value is so that it can be used many times.

    Note that materializing a value is an inherently destructive operation.
    Original values of mutable types such as lists and streams are emptied by
    the operation.

    Example:

    ls | materialize"#), Unknown)?;

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
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
