use std::collections::HashSet;
use crate::lang::command::Command;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, argument_error_legacy, CrushResult, mandate};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use crate::lang::{data::dict::Dict, value::ValueType};
use lazy_static::lazy_static;
use ordered_map::{Entry, OrderedMap};
use signature::signature;
use crate::data::table::ColumnVec;
use crate::util::replace::Replace;
use itertools::Itertools;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::this::This;

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
        Of::declare_method(&mut res, &path);
        Join::declare_method(&mut res, &path);
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
            [],
        );
        res.declare(
            full("collect"),
            collect,
            true,
            "dict:collect key_column _value_column",
            "Create a new dict by reading the specified columns from the input",
            None,
            Unknown,
            [],
        );
        res.declare(
            full("__setitem__"),
            setitem,
            false,
            "dict[key] = value",
            "Create a new mapping or replace an existing one",
            None,
            Unknown,
            [],
        );
        res.declare(
            full("__getitem__"),
            getitem,
            false,
            "dict[key]",
            "Return the value the specified key is mapped to",
            None,
            Unknown,
            [],
        );
        res.declare(
            full("contains"),
            contains,
            false,
            "dict:contains",
            "Returns true if the key is in the dict",
            None,
            Unknown,
            [],
        );
        res.declare(
            full("remove"),
            remove,
            false,
            "dict:remove key",
            "Remove a mapping from the dict",
            None,
            Unknown,
            [],
        );
        res
    };
}

#[signature(
__call__,
can_block = false,
output = Known(ValueType::Type),
short = "Returns a dict type with the specified key and value types.",
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
                let cfg: Call = Call::parse(context.arguments, &context.global_state.printer())?;
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
                    argument_error_legacy("Tried to set subtype on a dict that already has a subtype")
                }
            }
        },
        _ => argument_error_legacy("Invalid this, expected type dict"),
    }
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let t = context.this.r#type()?;
    if let ValueType::Dict(key_type, value_type) = t {
        if !key_type.is_hashable() {
            argument_error_legacy("Key type is not hashable")
        } else {
            context
                .output
                .send(Dict::new(*key_type, *value_type)?.into())
        }
    } else {
        argument_error_legacy("Expected a dict type as this value")
    }
}

#[signature(
of,
can_block = false,
output = Unknown,
short = "Create a new dict with the specified elements.",
)]
struct Of {}

fn of(mut context: CommandContext) -> CrushResult<()> {
    if context.arguments.len() % 2 == 1 {
        return argument_error_legacy("Expected an even number of arguments");
    }
    if context.arguments.len() == 0 {
        return argument_error_legacy("Expected at least one pair of arguments");
    }

    let mut key_types = HashSet::new();
    let mut value_types = HashSet::new();
    let mut entries = OrderedMap::new();

    let mut arg = context.remove_arguments().into_iter();

    while let Some((key, value)) = arg.next_tuple() {
        key_types.insert(key.value.value_type());
        value_types.insert(value.value.value_type());
        entries.insert(key.value, value.value);
    }

    if key_types.len() != 1 {
        return argument_error_legacy("Multiple key types specified in dict");
    }
    let key_type = key_types.drain().next().unwrap();
    let value_type = if value_types.len() == 1 {value_types.drain().next().unwrap() } else {ValueType::Any};

    context.output.send(Dict::new_with_data(key_type, value_type, entries)?.into())
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
    o.send(dict.get(&key).unwrap_or(Value::Empty))
}

fn contains(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    o.send(Value::Bool(dict.contains(&key)))
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

fn len(mut context: CommandContext) -> CrushResult<()> {
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
struct Clear {}

fn clear(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    d.clear();
    context.output.send(d.into())
}

#[signature(
clone,
can_block = false,
output = Unknown,
short = "Create a new dict with the same set of mappings as this one.",
)]
struct Clone {}

fn clone(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let d = context.this.dict()?;
    context.output.send(d.copy().into())
}

#[signature(
empty,
can_block = false,
output = Known(ValueType::Bool),
short = "True if there are no mappings in the dict.",
)]
struct Empty {}

fn empty(mut context: CommandContext) -> CrushResult<()> {
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

fn key_type(mut context: CommandContext) -> CrushResult<()> {
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

fn value_type(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Type(context.this.dict()?.value_type()))
}

fn collect(mut context: CommandContext) -> CrushResult<()> {
    let mut input = mandate(context.input.recv()?.stream()?, "Expected a stream")?;
    let input_type = input.types().to_vec();
    let mut res = OrderedMap::new();
    match context.arguments.len() {
        2 => {
            match (&context.arguments[0].value, &context.arguments[1].value) {
                (Value::String(key), Value::String(value)) => {
                    match (input_type.as_slice().find(key), input_type.as_slice().find(value)) {
                        (Ok(key_idx), Ok(value_idx)) => {
                            while let Ok(row) = input.read() {
                                let mut row = Vec::from(row);
                                res.insert(row.replace(key_idx, Value::Empty), row.replace(value_idx, Value::Empty));
                            }
                            context
                                .output
                                .send(Dict::new_with_data(input_type[key_idx].cell_type.clone(), input_type[value_idx].cell_type.clone(), res)?.into())
                        }
                        _ => argument_error("Columns not found", context.arguments[0].location)
                    }
                }
                _ => argument_error("Expected arguments of type string", context.arguments[0].location),
            }
        }
        _ => argument_error("Expected two arguments", context.arguments[0].location),
    }
}

#[signature(
join,
can_block = false,
output = Unknown,
short = "Create a new dict with the same set of mappings as this one.",
)]
struct Join {
    #[description("the dict instances to join.")]
    #[unnamed()]
    dicts: Vec<Dict>
}

fn join(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Join = Join::parse(context.remove_arguments(), context.global_state.printer())?;
    let mut dicts = cfg.dicts;

    if let Some(Value::Dict(this)) = context.this {
        dicts.insert(0, this);
    }
    let mut key_types = HashSet::new();
    let mut value_types = HashSet::new();

    let mut out = OrderedMap::new();

    for d in dicts.into_iter() {
        key_types.insert(d.key_type());
        value_types.insert(d.value_type());
        for e in d.elements() {
            match out.entry(e.0) {
                Entry::Occupied(_) => {}
                Entry::Vacant(v) => { v.insert(e.1) }
            }
        }
    }

    if key_types.len() != 1 {
        argument_error_legacy("Multiple key types specified in dict")
    } else {
        let key_type = key_types.drain().next().unwrap();
        let value_type = if value_types.len() == 1 { value_types.drain().next().unwrap() } else { ValueType::Any };

        context.output.send(Dict::new_with_data(key_type, value_type, out)?.into())
    }
}