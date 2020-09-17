use crate::lang::argument::column_names;
use crate::lang::command::CrushCommand;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::errors::{mandate, CrushResult};
use crate::lang::execution_context::ArgumentVector;
use crate::lang::execution_context::{CommandContext, This};
use crate::lang::data::scope::Scope;
use crate::lang::pipe::black_hole;
use crate::lang::data::table::ColumnType;
use crate::lang::value::ValueType;
use crate::lang::{data::r#struct::Struct, value::Value};
use crate::lang::ordered_string_map::OrderedStringMap;
use signature::signature;
pub mod binary;
pub mod dict;
pub mod duration;
pub mod file;
pub mod float;
pub mod glob;
pub mod integer;
pub mod list;
pub mod re;
pub mod scope;
pub mod string;
pub mod table;
pub mod table_stream;
pub mod time;

#[signature(
materialize,
can_block = true,
short = "Recursively convert all streams in io to materialized form",
example= "ls | materialize"
)]
struct Materialize {}

fn materialize(context: CommandContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize()?)
}

fn new(mut context: CommandContext) -> CrushResult<()> {
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

#[signature(
data,
can_block = false,
output = Known(ValueType::Struct),
short = "Construct a struct with the specified members",
long= "Example:",
long= "data foo=5 bar=\"baz\" false",
)]
struct Data {
    #[description("unnamed values.")]
    #[unnamed]
    unnamed: Vec<Value>,
    #[description("named values.")]
    #[named]
    named: OrderedStringMap<Value>,
}

fn data(context: CommandContext) -> CrushResult<()> {
    let mut names = column_names(&context.arguments);
    let arr: Vec<(String, Value)> = names
        .drain(..)
        .zip(context.arguments)
        .map(|(name, arg)| (name, arg.value))
        .collect::<Vec<(String, Value)>>();
    context.output.send(Value::Struct(Struct::new(arr, None)))
}

#[signature(
class,
can_block = false,
output = Known(ValueType::Struct),
short = "Create an empty new class",
long= "Example:",
long= "Point := class",
long= "Point:__init__ = {\n        |x:float y:float|\n        this:x = x\n        this:y = y\n    }",
long= "Point:len = {\n        ||\n        math:sqrt this:x*this:x + this:y*this:y\n    }",
long= "Point:__add__ = {\n        |other|\n        Point:new x=this:x+other:x y=this:y+other:y\n    }",
long= "p := (Point:new x=1.0 y=2.0)\n    p:len"
)]
struct Class {
    #[description("the type to convert the value to.")]
    parent: Option<Struct>,
}

fn class(context: CommandContext) -> CrushResult<()> {
    let cfg: Class = Class::parse(context.arguments, &context.global_state.printer())?;
    let scope = context.scope;
    let parent = cfg.parent.unwrap_or_else(|| scope.root_object());
    let res = Struct::new(vec![], Some(parent));
    context.output.send(Value::Struct(res))
}

pub fn column_types(columns: &OrderedStringMap<ValueType>) -> Vec<ColumnType> {
    columns.iter().map(|(key, value)| ColumnType::new(key, value.clone())).collect()
}

#[signature(
convert,
can_block = false,
short = "Convert the vale to the specified type"
)]
struct Convert {
    #[description("the value to convert.")]
    value: Value,
    #[description("the type to convert the value to.")]
    target_type: ValueType,
}

pub fn convert(context: CommandContext) -> CrushResult<()> {
    let cfg: Convert = Convert::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(cfg.value.convert(cfg.target_type)?)
}

#[signature(
__typeof__,
can_block = false,
output = Known(ValueType::Type),
short = "Return the type of the specified value.",
)]
struct TypeOf {
    #[description("the value to convert.")]
    value: Value,
}

pub fn __typeof__(context: CommandContext) -> CrushResult<()> {
    let cfg: TypeOf = TypeOf::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(Value::Type(cfg.value.value_type()))
}

fn class_set(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let value = context.arguments.value(1)?;
    let name = context.arguments.string(0)?;
    this.set(&name, value);
    context.output.send(Value::Empty())
}

fn class_get(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    let name = context.arguments.string(0)?;
    context.output.send(mandate(
        this.get(&name),
        format!("Unknown field {}", name).as_str(),
    )?)
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "types",
        Box::new(move |env| {
            let root =
                Struct::new(vec![
                    ("__setattr__".to_string(), Value::Command(CrushCommand::command(
                        class_set, false,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "__setattr__".to_string()],
                        "root:__setitem__ name:string value:any",
                        "Modify the specified field to hold the specified value",
                        None,
                        Known(ValueType::Empty),
                        vec![],
                    ))),
                    ("__getitem__".to_string(), Value::Command(CrushCommand::command(
                        class_get, false,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "__getitem__".to_string()],
                        "root:__getitem__ name:string",
                        "Return the value of the specified field",
                        None,
                        Unknown,
                        vec![],
                    ))),
                    ("__setitem__".to_string(), Value::Command(CrushCommand::command(
                        class_set, false,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "__setitem__".to_string()],
                        "root:__setitem__ name:string value:any",
                        "Modify the specified field to hold the specified value",
                        None,
                        Unknown,
                        vec![],
                    ))),
                    ("new".to_string(), Value::Command(CrushCommand::command(
                        new, true,
                        vec!["global".to_string(), "types".to_string(), "root".to_string(), "new".to_string()],
                        "root:new @unnamed @@named",
                        "Create a new instance of the specified type",
                        None,
                        Known(ValueType::Struct),
                        vec![],
                    ))),
                ], None);

            env.declare("root", Value::Struct(root))?;
            Data::declare(env)?;
            Class::declare(env)?;
            Convert::declare(env)?;
            TypeOf::declare(env)?;
            Materialize::declare(env)?;

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
