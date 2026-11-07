use crate::lang::any_sequence::AnySequence;
use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::command::OutputType::Known;
use crate::lang::command::{Command, CrushCommand, OutputType, PipeInputType};
use crate::lang::data::list::List;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::black_hole;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::state::this::This;
use crate::lang::value::ValueType;
use crate::lang::{data::r#struct::Struct, value::Value};
use signature::signature;

pub mod binary;
pub mod dict;
pub mod duration;
pub mod file;
pub mod float;
pub mod glob;
pub mod integer;
pub mod list;
pub mod one_of;
pub mod re;
pub mod scope;
pub mod string;
pub mod r#struct;
pub mod table;
pub mod table_input_stream;
pub mod table_output_stream;
pub mod time;
pub mod r#type;

/// Computes `materialize`'s real output type by delegating straight to
/// `ValueType::materialize` -- the exact same conversion the real command applies to the
/// actual value, just performed on its type alone.
fn materialize_dynamic_output(
    input: PipeInputType,
    _arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    match input {
        PipeInputType::Known(t) => Ok(OutputType::Known(t.materialize()?)),
        PipeInputType::None | PipeInputType::Unknown => Ok(OutputType::Unknown),
    }
}

#[signature(
    types.materialize,
    can_block = true,
    short = "Recursively convert all streams in io to materialized form",
    example = "# Put a table of files in the current directory into the variable $f",
    example = "$f := $(files | materialize)",
    example = "# Because we materialized the table stream into a table, counting the elements is not a destructive operation.",
    example = "$f | count",
    dynamic_output = materialize_dynamic_output,
)]
struct Materialize {}

fn materialize(context: CommandContext) -> CrushResult<()> {
    context.output.send(context.input.recv()?.materialize()?)
}

#[signature(
    types.definition,
    can_block = true,
    output = Known(ValueType::String),
    short = "Returns the definition of the specified closure as a text string",
    long = "Returns nothing if the command is not a closure",
    long = "",
    long = "Note that the outputted of the `definition` builtin may reformatted, including switching code between expression mode and command mode.",
    example = "# returns { $files --recursive '/'}",
    example = "$all_the_files := {files --recursive /}",
    example = "definition $all_the_files",
)]
struct Definition {
    #[description("the closure to show the definition of.")]
    command: Command,
}

fn definition(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Definition::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(
        cfg.command
            .definition()
            .map(|s| Value::from(s))
            .unwrap_or(Value::Empty),
    )
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    let parent = context.this.clone().r#struct()?;
    let res = Struct::empty(Some(parent));
    let o = context.output;

    // Call constructor if one exists. Propagate a failure rather than warning and
    // returning the struct anyway -- a half-initialized instance (with whatever fields
    // __init__ managed to set before failing) is worse than no instance at all, and
    // every other command's failure already halts the calling script, so `new` should
    // behave the same way instead of being the one place a failure is fully absorbed.
    if let Some(Value::Command(c)) = res.get("__init__") {
        context.output = black_hole();
        context.this = Some(Value::Struct(res.clone()));
        c.eval(context)?;
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
    #[description("the parent type to inherit members from (if any).")]
    parent: Option<Struct>,
}

fn class(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Class = Class::parse(context.remove_arguments(), &context.global_state.printer())?;
    let scope = context.scope;
    let parent = cfg.parent.unwrap_or_else(|| scope.root_object());
    let res = Struct::empty(Some(parent));
    context.output.send(Value::Struct(res))
}

pub fn column_types(columns: &OrderedStringMap<ValueType>) -> Vec<ColumnType> {
    columns
        .iter()
        .map(|(key, value)| {
            ColumnType::new_from_string(key.clone(), value.clone(), "a column defined by the caller.".to_string())
        })
        .collect()
}

#[signature(
    types.convert,
    can_block = false,
    dynamic_output = convert_dynamic_output,
    short = "Convert the value to the specified type",
    long = "Converting a value to the type it already holds always works and returns the original value. Most other conversions take the input value, convert it to a string and then attempt to parse that string as the desired type.",
    long = "",
    long = "The following short cut conversions exist that do not go via a string representation:",
    long = "* `$float` to `$integer` the value is truncated to its integer part.",
    long = "* `$integer` to `$bool` 0 is false, all other values are true.",
    long = "",
    long = "If `convert`'s input is a pipeline, `convert` converts the value in the",
    long = "pipeline. Otherwise, `convert` requires a value to be provided as an",
    long = "argument, and converts that value instead.",
    example = "convert $integer 1.8",
)]
struct Convert {
    #[description("the type to convert the value to.")]
    target_type: ValueType,
    #[description("the value to convert.")]
    value: Option<Value>,
}

/// `convert`'s output is always exactly `target_type` -- the one thing it can't predict
/// is whether the actual conversion will succeed at runtime, which doesn't change the
/// *type* either way.
fn convert_dynamic_output(
    _input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let mut seen = 0usize;
    for arg in arguments {
        let is_target_type = match &arg.argument_type {
            Some(name) => name == "target_type",
            None => {
                let matched = seen == 0;
                seen += 1;
                matched
            }
        };
        if !is_target_type {
            continue;
        }
        return match &arg.value {
            ArgumentValue::Value(Value::Type(t)) => Ok(OutputType::Known(t.clone())),
            ArgumentValue::Unknown | ArgumentValue::Type(_) => Ok(OutputType::Unknown),
            _ => arg.error("`target_type` must be given a type"),
        };
    }
    Ok(OutputType::Unknown)
}

pub fn convert(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Convert = Convert::parse(context.remove_arguments(), &context.global_state.printer())?;
    let value = context.input.recv_or(cfg.value)?;
    context.output.send(value.convert(cfg.target_type)?)
}

#[signature(
    types.r#typeof,
    can_block = false,
    output = Known(ValueType::Type),
    short = "Return the type of the specified value.",
    long = "If `typeof`'s input is a pipeline, `typeof` returns the type of the value in",
    long = "the pipeline. Otherwise, `typeof` requires a value to be provided as an",
    long = "argument, and returns the type of that value instead.",
    example = "# Returns float",
    example = "typeof 1.8",
)]
struct TypeOf {
    #[description("the value to provide the type of.")]
    value: Option<Value>,
}

pub fn r#typeof(mut context: CommandContext) -> CrushResult<()> {
    let cfg: TypeOf = TypeOf::parse(context.remove_arguments(), &context.global_state.printer())?;
    let value = context.input.recv_or(cfg.value)?;
    context.output.send(Value::Type(value.value_type()))
}

#[signature(
    types.root.__setitem__,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Modify the specified field to hold the specified value.",
)]
struct SetItem {
    #[description("the name of the field to set the value of.")]
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
    #[description("the name of the field to set the value of.")]
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
    types.like,
    can_block = true,
    output = Known(ValueType::Bool),
    short = "Check if the specified value matches one or more patterns.",
    long = "A pattern can be a string (exact match), a glob, a regular expression, a type",
    long = "(checks `value`'s type), or any other value that implements an `__is__`",
    long = "method -- `like` simply calls it. If multiple patterns are given, they're",
    long = "checked in order and `like` returns true as soon as one of them matches.",
    long = "",
    long = "In expression mode, a single pattern can also be checked via the `=~`",
    long = "operator (or `!~` for the negation).",
    example = "# Match the string \"fooo\" against the regex ^(ooo)",
    example = "like fooo ^(ooo)",
    example = "# Same, in expression mode",
    example = "(fooo =~ ^(ooo))",
    example = "# A value matches if it equals any of several patterns",
    example = "like fooo ^(ooo) *.txt \"fooo\"",
)]
struct Like {
    #[description("the value to test.")]
    value: Value,
    #[description("the pattern(s) to test the value against.")]
    #[unnamed()]
    pattern: Vec<Value>,
}

fn like(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Like::parse(context.remove_arguments(), context.global_state.printer())?;
    if cfg.pattern.is_empty() {
        return command_error("Expected at least one pattern.");
    }
    for pattern in &cfg.pattern {
        if pattern.is(&cfg.value, &context)? {
            return context.output.send(Value::from(true));
        }
    }
    context.output.send(Value::from(false))
}

#[signature(
    types.val,
    can_block = false,
    short = "Return value",
    output = Known(ValueType::Any),
    long = "This command is useful if you want to pass a command as input in a pipeline instead of executing it. It is different from the echo command in that val sends the value through the pipeline, whereas echo prints it to screen.",
    example = "val $val",
)]
struct Val {
    #[description("the value to pass as output.")]
    value: Value,
}

pub fn val(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Val = Val::parse(context.remove_arguments(), &context.global_state.printer())?;
    let res = context.output.send(cfg.value);
    res
}

#[signature(
    types.dir,
    can_block = false,
    short = "List the member names of a value.",
    output = Known(ValueType::List(Box::from(ValueType::String))),
    long = "Works on any value -- a struct's own fields, a scope's local variables, or",
    long = "(for anything else, including a type value like `$float`) the methods its",
    long = "type declares.",
    long = "",
    long = "If `dir`'s input is a pipeline, `dir` lists the members of the value in the",
    long = "pipeline. Otherwise, `dir` requires a value to be provided as an argument,",
    long = "and lists the members of that value instead.",
    long = "",
    long = "Pair with `member` to fetch one of the listed names -- `member`'s own name",
    long = "argument, unlike the `:` operator, can be a runtime value instead of a fixed",
    long = "word in the source, so the two together let you enumerate and read members",
    long = "whose names aren't known ahead of time.",
    example = "dir .",
    example = "# The full help text of every method float has",
    example = "dir $float | each {|$name| help ($float | member $name)}",
)]
struct Dir {
    #[description("the value to list the members of.")]
    value: Option<Value>,
}

pub fn dir(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Dir = Dir::parse(context.remove_arguments(), &context.global_state.printer())?;
    let value = context.input.recv_or(cfg.value)?;
    context.output.send(
        List::new(
            ValueType::String,
            value
                .fields()
                .drain(..)
                .map(|n| Value::from(n))
                .collect::<Vec<_>>(),
        )
        .into(),
    )
}

#[signature(
    types.member,
    can_block = false,
    short = "Extract one named member from a value.",
    long = "Works like the `:` member operator, except the member name is a runtime",
    long = "value (e.g. a variable) rather than a fixed word in the source -- use this",
    long = "when the name to look up isn't known until the script runs. Pair with `dir`",
    long = "to discover a value's member names first.",
    long = "",
    long = "If `member`'s input is a pipeline, `member` extracts the named member from",
    long = "the value in the pipeline. Otherwise, `member` requires a value to be",
    long = "provided as an argument, and extracts the named member from that value",
    long = "instead.",
    example = "$uri := \"https://raw.githubusercontent.com/liljencrantz/crush/refs/heads/master/example_data/dinosaurs.json\"",
    example = "http $uri | member body | json:from",
    example = "# member also takes its value as an argument instead of a pipe",
    example = "member __neg__ 5",
    example = "# dir lists a value's member names; member fetches one by name -- together",
    example = "# they let you enumerate members whose names aren't known ahead of time",
    example = "for name=$(dir 5) { echo (member $name 5) }",
)]
struct Member {
    #[description("the member to extract.")]
    field: String,
    #[description("the value to extract the member from.")]
    value: Option<Value>,
}

fn member(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Member = Member::parse(context.remove_arguments(), &context.global_state.printer())?;
    let value = context.input.recv_or(cfg.value)?;
    let result = value.field(&cfg.field)?.ok_or_else(|| {
        format!(
            "Missing field `{}` in value of type `{}`",
            cfg.field,
            value.value_type()
        )
    })?;
    context.output.send(result)
}

static MEMBERS_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("name", ValueType::String, "the column's name."),
    ColumnType::new("type", ValueType::Type, "the column's type."),
];

#[signature(
    types.members,
    can_block = true,
    output = Known(ValueType::table_input_stream(&MEMBERS_OUTPUT_TYPE)),
    short = "List the columns of any streamable input value as name/type pairs.",
    long = "Works on anything that can be read as a stream of rows -- a table, a list, a",
    long = "dict, a struct, a scope -- and reports that stream's shape without consuming",
    long = "any of its actual rows.",
    example = "$my_dict | members",
)]
struct Members {}

fn members(mut context: CommandContext) -> CrushResult<()> {
    Members::parse(context.remove_arguments(), &context.global_state.printer())?;
    let input = context.input_stream()?;
    let output = context.initialize_output(&MEMBERS_OUTPUT_TYPE)?;
    for ct in input.types() {
        output.send(Row::new(vec![
            Value::from(ct.name().to_string()),
            Value::Type(ct.cell_type.clone()),
        ]))?;
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "types",
        "Crush built in types and type related builtins.",
        Some(
            "Every value type Crush knows about, and the methods you can call on instances of \
             it -- `string`, `integer`, `list`, `dict`, `time`, `duration`, `table`, `re`, \
             `glob`, and the rest, each its own sub-namespace of methods (e.g. \
             `\"hello\":upper`, `$my_list:push value`). Also home to the type system's own \
             cross-cutting machinery: `class` (defining new types), `convert`, `typeof`, \
             `like` (pattern matching), `materialize`, and `dir`/`member`/`members` (generic \
             value introspection). Imported into the global scope, so both `types:string` and \
             bare `string` refer to the same type."
                .to_string(),
        ),
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
                        None,
                    ))),
                ], None);

            env.declare("root", Value::Struct(root))?;
            Class::declare(env)?;
            Convert::declare(env)?;
            TypeOf::declare(env)?;
            Like::declare(env)?;
            Materialize::declare(env)?;
            Definition::declare(env)?;
            Val::declare(env)?;
            Dir::declare(env)?;
            Member::declare(env)?;
            Members::declare(env)?;

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
            env.declare("list", Value::Type(ValueType::List(Box::from(ValueType::Any))))?;
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
            env.declare("one_of", Value::Type(ValueType::OneOf(AnySequence::Slice(&[]))))?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use std::sync::Arc;

    fn unnamed_arg(value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: None,
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    fn named_arg(name: &str, value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: Some(name.to_string()),
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    #[test]
    fn convert_with_positional_target_type_is_known() {
        let args = [
            unnamed_arg(ArgumentValue::Value(Value::Type(ValueType::Integer))),
            unnamed_arg(ArgumentValue::Value(Value::from("1.8"))),
        ];
        let out = convert_dynamic_output(PipeInputType::None, &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Integer)));
    }

    #[test]
    fn convert_with_named_target_type_is_known() {
        let args = [named_arg(
            "target_type",
            ArgumentValue::Value(Value::Type(ValueType::String)),
        )];
        let out = convert_dynamic_output(PipeInputType::None, &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::String)));
    }

    #[test]
    fn convert_with_non_type_target_is_an_error() {
        let args = [unnamed_arg(ArgumentValue::Value(Value::from("oops")))];
        assert!(convert_dynamic_output(PipeInputType::None, &args).is_err());
    }

    #[test]
    fn convert_with_no_target_type_is_unknown() {
        let out = convert_dynamic_output(PipeInputType::None, &[]).unwrap();
        assert!(matches!(out, OutputType::Unknown));
    }

    #[test]
    fn materialize_resolves_table_input_stream_to_table() {
        let out = materialize_dynamic_output(
            PipeInputType::Known(ValueType::TableInputStream(vec![])),
            &[],
        )
        .unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Table(_))));
    }

    #[test]
    fn materialize_of_unknown_input_is_unknown() {
        let out = materialize_dynamic_output(PipeInputType::Unknown, &[]).unwrap();
        assert!(matches!(out, OutputType::Unknown));
    }
}
