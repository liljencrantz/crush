use crate::lang::command::Command;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, CommandContext, This};
use crate::lang::value::Value;
use crate::lang::{dict::Dict, value::ValueType};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "dict", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "dict"];
        Len::declare_method(&mut res, &path);
        Empty::declare_method(&mut res, &path);
        Call::declare_method(&mut res, &path);
        Clone::declare_method(&mut res, &path);
        Clear::declare_method(&mut res, &path);
        KeyType::declare_method(&mut res, &path);
        ValueTypeMethod::declare_method(&mut res, &path);
        res.declare(
            full("new"),
            new,
            false,
            "dict:new",
            "Construct a new dict",
            Some(
                r#"    Examples:
    my_dict := (dict string integer):new"#,
            ),
            Unknown,
        );
        res.declare(
            full("__setitem__"),
            setitem,
            false,
            "dict[key] = value",
            "Create a new mapping or replace an existing one",
            None,
            Unknown,
        );
        res.declare(
            full("__getitem__"),
            getitem,
            false,
            "dict[key]",
            "Return the value the specified key is mapped to",
            None,
            Unknown,
        );
        res.declare(
            full("remove"),
            remove,
            false,
            "dict:remove key",
            "Remove a mapping from the dict",
            None,
            Unknown,
        );
        res
    };
}

#[signature(
__call__,
can_block = false,
output = Known(ValueType::Type),
short = "Returns a dict type with the specific key and value types.",
)]
struct Call {
    #[description("the type of the keys in the dict.")]
    key_type: ValueType,
    #[description("the type of the values in the dict.")]
    value_type: ValueType,
}

fn __call__(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::Dict(t1, t2) => match (*t1, *t2) {
            (ValueType::Empty, ValueType::Empty) => {
                let cfg: Call = Call::parse(context.arguments, &context.printer)?;
                context.output.send(Value::Type(ValueType::Dict(
                    Box::new(cfg.key_type),
                    Box::new(cfg.value_type),
                )))
            }
            (t1, t2) => {
                if context.arguments.is_empty() {
                    context
                        .output
                        .send(Value::Type(ValueType::Dict(Box::from(t1), Box::from(t2))))
                } else {
                    argument_error("Tried to set subtype on a dict that already has a subtype")
                }
            }
        },
        _ => argument_error("Invalid this, expected type dict"),
    }
}

fn new(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let t = context.this.r#type()?;
    if let ValueType::Dict(key_type, value_type) = t {
        if !key_type.is_hashable() {
            argument_error("Key type is not hashable")
        } else {
            context
                .output
                .send(Value::Dict(Dict::new(*key_type, *value_type)))
        }
    } else {
        argument_error("Expected a dict type as this value")
    }
}

fn setitem(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let dict = context.this.dict()?;
    let value = context.arguments.value(1)?;
    let key = context.arguments.value(0)?;
    dict.insert(key, value)
}

fn getitem(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.get(&key).map(|c| o.send(c));
    Ok(())
}

fn remove(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    dict.remove(&key).map(|c| o.send(c));
    Ok(())
}

#[signature(
len,
can_block = false,
output = Known(ValueType::Integer),
short = "The number of mappings in the dict.",
)]
struct Len {}

fn len(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(context.this.dict()?.len() as i128))
}

#[signature(
clear,
can_block = false,
output = Unknown,
short = "Remove all mappings from this dict.",
)]
struct Clear {
}

fn clear(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    d.clear();
    context.output.send(Value::Dict(d))
}

#[signature(
clone,
can_block = false,
output = Unknown,
short = "Create a new dict with the same set of mappings as this one.",
)]
struct Clone {
}

fn clone(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    context.output.send(Value::Dict(d.copy()))
}

#[signature(
empty,
can_block = false,
output = Known(ValueType::Bool),
short = "True if there are no mappings in the dict.",
)]
struct Empty {}

fn empty(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.dict()?.len() == 0))
}

#[signature(
key_type,
can_block = false,
output = Known(ValueType::Type),
short = "the type of the keys in this dict.",
)]
struct KeyType {}

fn key_type(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Type(context.this.dict()?.key_type()))
}

#[signature(
value_type,
can_block = false,
output = Known(ValueType::Type),
short = "the type of the values in this dict.",
)]
struct ValueTypeMethod {}

fn value_type(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Type(context.this.dict()?.value_type()))
}
