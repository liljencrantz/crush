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

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Len::declare_method(&mut res);
        Empty::declare_method(&mut res);
        Call::declare_method(&mut res);
        Clone::declare_method(&mut res);
        Clear::declare_method(&mut res);
        KeyType::declare_method(&mut res);
        ValueTypeMethod::declare_method(&mut res);
        Of::declare_method(&mut res);
        Join::declare_method(&mut res);
        Join::declare_method(&mut res);
        New::declare_method(&mut res);
        Collect::declare_method(&mut res);
        Remove::declare_method(&mut res);
        Contains::declare_method(&mut res);
        SetItem::declare_method(&mut res);
        GetItem::declare_method(&mut res);

        res
    };
}

#[signature(
    __call__,
    can_block = false,
    output = Known(ValueType::Type),
    short = "Returns a dict type with the specified key and value types.",
    path = ("types", "dict"),
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
#[signature(
    new,
    can_block = false,
    output = Known(ValueType::Dict(Box::from(ValueType::Empty), Box::from(ValueType::Empty))),
    short = "Create an empty new dict.",
    long = "This method takes no arguments, but must not be called on a raw dict type. You must call it on a parametrized dict type, like $(dict $string $string)",
    example = "my_dict := $($(dict $string $integer):new)",
    path = ("types", "dict"),
)]
struct New {}

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
    path = ("types", "dict"),
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
    let value_type = if value_types.len() == 1 { value_types.drain().next().unwrap() } else { ValueType::Any };

    context.output.send(Dict::new_with_data(key_type, value_type, entries)?.into())
}

#[signature(
    __setitem__,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Create a new mapping or replace an existing one.",
    path = ("types", "dict"),
)]
struct SetItem {
    #[description("the key of the value to set.")]
    key: Value,
    #[description("the new value.")]
    value: Value,
}


fn __setitem__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let dict = context.this.dict()?;
    let value = context.arguments.value(1)?;
    let key = context.arguments.value(0)?;
    dict.insert(key, value)
}

#[signature(
    __getitem__,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Return the value mapped to the specified key of the dict.",
    path = ("types", "dict"),
)]
struct GetItem {
    #[description("the key of the value to get.")]
    key: Value,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let key = context.arguments.value(0)?;
    let o = context.output;
    o.send(dict.get(&key).unwrap_or(Value::Empty))
}

#[signature(
    contains,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "Returns whether the given key is in the dict",
    path = ("types", "dict"),
)]
struct Contains {
    #[description("the value to check.")]
    key: Value,
}

fn contains(mut context: CommandContext) -> CrushResult<()> {
    let dict = context.this.dict()?;
    let cfg: Contains = Contains::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Bool(dict.contains(&cfg.key)))
}

#[signature(
    remove,
    can_block = false,
    output = Unknown,
    short = "Remove a mapping from the dict and return the value, or nothing if there was no value in the dict",
    path = ("types", "dict"),
)]
struct Remove {
    #[description("the value to remove.")]
    key: Value,
}

fn remove(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let dict = context.this.dict()?;
    let cfg: Remove = Remove::parse(context.remove_arguments(), &context.global_state.printer())?;
    let o = context.output;
    dict.remove(&cfg.key).map(|c| o.send(c));
    Ok(())
}

#[signature(
    len,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "The number of mappings in the dict.",
    path = ("types", "dict"),
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
    path = ("types", "dict"),
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
    path = ("types", "dict"),
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
    path = ("types", "dict"),
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
    path = ("types", "dict"),
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
    path = ("types", "dict"),
)]
struct ValueTypeMethod {}

fn value_type(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Type(context.this.dict()?.value_type()))
}

#[signature(
    collect,
    can_block = true,
    output = Known(ValueType::Dict(Box::from(ValueType::Empty), Box::from(ValueType::Empty))),
    short = "Create a new dict by reading the specified columns from the input.",
    path = ("types", "dict"),
)]
struct Collect {
    #[description("the name of the column to use as key")]
    key_column: String,
    #[description("the name of the column to use as value")]
    value_column: String,
}

fn collect(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Collect = Collect::parse(context.remove_arguments(), context.global_state.printer())?;
    let mut input = mandate(context.input.recv()?.stream()?, "Expected a stream")?;
    let input_type = input.types().to_vec();
    let mut res = OrderedMap::new();
    match (input_type.as_slice().find(&cfg.key_column), input_type.as_slice().find(&cfg.value_column)) {
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

#[signature(
    join,
    can_block = false,
    output = Unknown,
    short = "Create a new dict with the same set of mappings as this one.",
    path = ("types", "dict"),
)]
struct Join {
    #[description("the dict instances to join.")]
    #[unnamed()]
    dicts: Vec<Dict>,
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
