use crate::lang::command::{Command, CrushCommand};
use crate::lang::command::OutputType::Known;
use crate::lang::data::table::ColumnType;
use crate::lang::errors::CrushResult;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::black_hole;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::state::this::This;
use crate::lang::value::ValueType;
use crate::lang::{data::r#struct::Struct, value::Value};
use signature::signature;
use crate::lang::signature::patterns::Patterns;

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
pub mod table_input_stream;
pub mod table_output_stream;
pub mod time;
pub mod r#struct;
pub mod one_of;

#[signature(
    types.materialize,
    can_block = true,
    short = "Recursively convert all streams in io to materialized form",
    example = "# Put a table of files in the current directory into the variable $f",
    example = "$f := $(files | materialize)",
    example = "# Because we materialized the table stream into a table, counting the elements is not destructive",
    example = "$f | count",
)]
struct Materialize {}

fn materialize(context: CommandContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize()?)
}

#[signature(
    types.definition,
    can_block = true,
    output = Known(ValueType::String),
    short = "Show the definition of the specified closure",
    long = "Returns nothing if the command is not a closure",
    long = "",
    long = "Note that the outputted may be significantly reformatted, including switching code between expression mode and command mode.",
    example = "$all_the_files := {files --recursive /}",
    example = "definition $all_the_files",
)]
struct Definition {
    command: Command,
}

fn definition(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Definition::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(cfg.command.definition().map(|s| Value::from(s)).unwrap_or(Value::Empty))
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    let parent = context.this.clone().r#struct()?;
    let res = Struct::empty(Some(parent));
    let o = context.output;

    // Call constructor if one exists
    if let Some(Value::Command(c)) = res.get("__init__") {
        let p = context.global_state.printer().clone();
        context.output = black_hole();
        context.this = Some(Value::Struct(res.clone()));
        p.handle_error(c.eval(context));
    }
    o.send(Value::Struct(res))
}

#[signature(
    types.class,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "Create an empty new class",
    example = "# Create a class that represents a point in 2D space",
    example = "$Point := $(class)",
    example = "$Point:__short_help__ = \"A point in 2D space\"",
    example = "$Point:__signature__ = \"class Point\"",
    example = "$Point:__long_help__ = \"Uses floating point numbers to represent the x and y coordinates\"",
    example = "$Point:__example__ = \"$p := ($Point.new(1.0, 2.0))\"",
    example = "# Constructor takes two arguments, x and y",
    example = "Point:__init__ = {",
    example = "  |$x:$float $y:$float|",
    example = "  $this:x = $x",
    example = "  $this:y = $y",
    example = "}",
    example = "",
    example = "Point:len = {",
    example = "  |",
    example = "  short_help = \"Returns the distance from the origin\"",
    example = "  |",
    example = "  ($math.sqrt($this.x*$this.x + $this.y*$this.y))",
    example = "}",
    example = "# Overload the `+` operator to add two points. (Only available in expression mode)",
    example = "Point:__add__ = {",
    example = "  |",
    example = "  short_help = \"Add two points together\"",
    example = "  $other : $struct \"the other point.\"",
    example = "  |",
    example = "  ($Point.new(x=($this.x+$other.x), y=($this.y+$other.y)))",
    example = "}",
    example = "$p := $($Point:new x=1.0 y=2.0)",
    example = "$p:len",
    example = "$p2 := ($Point.new(x=-1.0, y=2.0))",
    example = "$p3 := ($p + $p2)",
)]
struct Class {
    #[description("the type to convert the value to.")]
    parent: Option<Struct>,
}

fn class(context: CommandContext) -> CrushResult<()> {
    let cfg: Class = Class::parse(context.arguments, &context.global_state.printer())?;
    let scope = context.scope;
    let parent = cfg.parent.unwrap_or_else(|| scope.root_object());
    let res = Struct::empty(Some(parent));
    context.output.send(Value::Struct(res))
}

pub fn column_types(columns: &OrderedStringMap<ValueType>) -> Vec<ColumnType> {
    columns
        .iter()
        .map(|(key, value)| ColumnType::new_from_string(key.clone(), value.clone()))
        .collect()
}

#[signature(
    types.convert,
    can_block = false,
    short = "Convert the vale to the specified type",
    long = "Converting a value to the type it already holds always works and returns the original value. Most other conversions take the input value, convert it to a string and then attempt to parse that string as the desired type.",
    long = "",
    long = "The following short cut conversions exist that do not go via a string representation:",
    long = "* `$float` to `$integer` the value is truncated to its integer part.",
    long = "* `$integer` to `$bool` 0 is false, all other values are true.",
    example = "convert 1.8 $integer",
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
    types.r#typeof,
    can_block = false,
    output = Known(ValueType::Type),
    short = "Return the type of the specified value.",
    example = "typeof 1.8",
    example = "# returns \"float\"",
)]
struct TypeOf {
    #[description("the value to convert.")]
    value: Value,
}

pub fn r#typeof(context: CommandContext) -> CrushResult<()> {
    let cfg: TypeOf = TypeOf::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(Value::Type(cfg.value.value_type()))
}

#[signature(
    types.root.__setitem__,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Modify the specified field to hold the specified value.",
)]
struct SetItem {
    #[description("the name of the field to get the value of.")]
    name: String,
    #[description("the new value for the field.")]
    value: Value,
}

fn __setitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg = SetItem::parse(context.remove_arguments(), context.global_state.printer())?;
    let this = context.this.r#struct()?;
    this.set(&cfg.name, cfg.value);
    context.output.send(Value::Empty)
}

#[signature(
    types.root.__setattr__,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Modify the specified field to hold the specified value.",
)]
struct SetAttr {
    #[description("the name of the field to get the value of.")]
    name: String,
    #[description("the new value for the field.")]
    value: Value,
}

fn __setattr__(mut context: CommandContext) -> CrushResult<()> {
    let cfg = SetAttr::parse(context.remove_arguments(), context.global_state.printer())?;
    let this = context.this.r#struct()?;
    this.set(&cfg.name, cfg.value);
    context.output.send(Value::Empty)
}

#[signature(
    types.root.__getitem__,
    can_block = false,
    output = Known(ValueType::Any),
    short = "Return the value of the specified field.",
)]
struct GetItem {
    #[description("the name of the field to get the value of.")]
    name: String,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg = GetItem::parse(context.remove_arguments(), context.global_state.printer())?;
    let this = context.this.r#struct()?;
    context.output.send(
        this.get(&cfg.name)
            .ok_or(format!("Unknown field {}", cfg.name).as_str())?,
    )
}

#[signature(
    types.r#match,
    can_block = true,
    output = Known(ValueType::Bool),
    short = "Check if the specified value matches the pattern.",
    long = "The pattern can be another string, a glob or a regular expression. If multiple patterns are specified, they are checked in order and if any of them match, then true is returned.",
    long = "",
    long = "Under the hood, matching is performed by calling the `match` method on the value. `$string`, `$regex` and `$glob` all implement this method. You can create custom matching objects that are compatible with the match command by implementing this method yourself.",
    long = "",
    long = "In expression mode, this method can be used via the the `=~` operator.",
    example = "# Match the string \"foo\" against the regex ooo",
    example = "match fooo ^(ooo)",
    example = "# Match the string \"foo\" against the regex ooo using the expression mode",
    example = "(fooo =~ ^(ooo))",
)]
struct Match {
    #[description("the value.")]
    value: String,
    #[description("the pattern.")]
    pattern: Patterns,
}

fn r#match (mut context: CommandContext) -> CrushResult<()> {
    let cfg = Match::parse(context.remove_arguments(), context.global_state.printer())?;
    context.output.send(Value::from(cfg.pattern.test(&cfg.value)))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "types",
        "Crush built in types and type related builtins.",
        Box::new(move |env| {
            let root =
                Struct::new(vec![
                    ("__setattr__", Value::Command(SetAttr::create_command())),
                    ("__getitem__", Value::Command(GetItem::create_command())),
                    ("__setitem__", Value::Command(SetItem::create_command())),
                    ("new", Value::Command(<dyn CrushCommand>::command(
                        new, true,
                        &["global", "types", "root", "new"],
                        "root:new @unnamed @@named",
                        "Create a new instance of the specified type",
                        Some("The `new` method ignores any arguments and returns a new instance of the type. If there parent struct has a `__init__` method, it will be called with all the named and unnnamed arguments passed in."),
                        Known(ValueType::Struct),
                        [],
                    ))),
                ], None);

            env.declare("root", Value::Struct(root))?;
            Class::declare(env)?;
            Convert::declare(env)?;
            TypeOf::declare(env)?;
            Match::declare(env)?;
            Materialize::declare(env)?;
            Definition::declare(env)?;

            env.declare("file", Value::Type(ValueType::File))?;
            env.declare("type", Value::Type(ValueType::Type))?;
            env.declare("any", Value::Type(ValueType::Any))?;
            env.declare("bool", Value::Type(ValueType::Bool))?;
            env.declare("command", Value::Type(ValueType::Command))?;
            env.declare("scope", Value::Type(ValueType::Scope))?;
            env.declare("binary", Value::Type(ValueType::Binary))?;
            env.declare("binary_stream", Value::Type(ValueType::BinaryInputStream))?;
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
            env.declare("table_input_stream", Value::Type(ValueType::TableInputStream(vec![])))?;
            env.declare("table_output_stream", Value::Type(ValueType::TableOutputStream(vec![])))?;
            env.declare("struct", Value::Type(ValueType::Struct))?;
            env.declare("one_of", Value::Type(ValueType::OneOf(vec![])))?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
