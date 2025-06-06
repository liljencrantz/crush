use std::clone::Clone;
use std::sync::OnceLock;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{argument_error_legacy, CrushResult, data_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use crate::lang::{command::Command, data::list::List, value::ValueType};
use ordered_map::OrderedMap;
use signature::signature;
use crate::data::table::ColumnVec;
use crate::lang::pipe::{Stream, ValueSender};
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::this::This;
use crate::util::replace::Replace;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Len::declare_method(&mut res);
        Empty::declare_method(&mut res);
        Clear::declare_method(&mut res);
        Push::declare_method(&mut res);
        Pop::declare_method(&mut res);
        Peek::declare_method(&mut res);
        Remove::declare_method(&mut res);
        Insert::declare_method(&mut res);
        Truncate::declare_method(&mut res);
        CloneCmd::declare_method(&mut res);
        Of::declare_method(&mut res);
        Collect::declare_method(&mut res);
        New::declare_method(&mut res);
        GetItem::declare_method(&mut res);
        SetItem::declare_method(&mut res);
        Repeat::declare_method(&mut res);
        Call::declare_method(&mut res);
        Slice::declare_method(&mut res);

        res
    })
}

#[signature(
    types.list.repeat,
    can_block = false,
    short = "Create a list containing the same value multiple times",
)]
struct Repeat {
    #[description("the value to put into the list.")]
    item: Value,
    #[description("the number of times to put it in the list.")]
    times: usize,
}

fn repeat(context: CommandContext) -> CrushResult<()> {
    let cfg: Repeat = Repeat::parse(context.arguments, &context.global_state.printer())?;
    let mut l = Vec::with_capacity(cfg.times as usize);
    for _i in 0..cfg.times {
        l.push(cfg.item.clone());
    }
    context
        .output
        .send(List::new(cfg.item.value_type(), l).into())
}

#[signature(
    types.list.__call__,
    can_block = false,
    output = Known(ValueType::Type),
    short = "Returns a list type with the specified value type.",
)]
struct Call {
    #[description("the type of the values in the list.")]
    value_type: ValueType,
}

fn __call__(mut context: CommandContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::List(c) => match *c {
            ValueType::Empty => {
                let cfg: Call = Call::parse(context.arguments, &context.global_state.printer())?;
                context.output.send(Value::Type(ValueType::List(Box::new(
                    cfg.value_type))))
            }
            c => {
                if context.arguments.is_empty() {
                    context
                        .output
                        .send(Value::Type(ValueType::List(Box::from(c))))
                } else {
                    argument_error_legacy(
                        format!(
                            "Tried to set subtype on a list that already has the subtype {}",
                            c.to_string()
                        )
                        .as_str(),
                    )
                }
            }
        },
        _ => argument_error_legacy("Invalid this, expected type list"),
    }
}
#[signature(
    types.list.of,
    can_block = false,
    output = Known(ValueType::List(Box::from(ValueType::Any))),
    short = "Create a new list containing the supplied elements.",
)]
struct Of {
    #[description("the elements of the new list.")]
    #[unnamed()]
    values: Vec<Value>,
}

fn of(context: CommandContext) -> CrushResult<()> {
    let cfg: Of = Of::parse(context.arguments, &context.global_state.printer())?;
    match cfg.values.len() {
        0 => argument_error_legacy("Expected at least one argument"),
        _ => context.output.send(List::new_without_type(cfg.values).into()),
    }
}

#[signature(
    types.list.collect,
    can_block = false,
    output = Known(ValueType::List(Box::from(ValueType::Any))),
    short = "Create a new list by reading a column from the input.",
    long= "If no elements are supplied as arguments, input must be a stream with exactly one column.",
)]
struct Collect {
    column: Option<String>,
}

fn collect_internal(mut input: Stream, idx: usize, value_type: ValueType, output: ValueSender) -> CrushResult<()> {
    let mut lst = Vec::new();
    while let Ok(row) = input.read() {
        lst.push(Vec::from(row).replace(idx, Value::Empty));
    }

    output.send(List::new(value_type, lst).into())
}

fn collect(context: CommandContext) -> CrushResult<()> {
    let cfg: Collect = Collect::parse(context.arguments, &context.global_state.printer())?;
    let input = context.input.recv()?.stream()?.ok_or("Expected a stream")?;
    let input_type = input.types().to_vec();
    match (input_type.len(), cfg.column) {
        (_, Some(name)) =>
            match input_type.as_slice().find(&name) {
                Ok(idx) =>
                    collect_internal(input, idx, input_type[idx].cell_type.clone(), context.output),
                _ => data_error(format!("Column {} not found", name))
            }

        (1, None) =>
            collect_internal(input, 0, input_type[0].cell_type.clone(), context.output),

        _ =>  data_error("Expected either input with exactly one column or an argument specifying which column to pick"),
    }
}

#[signature(
    types.list.new,
    can_block = false,
    output = Known(ValueType::List(Box::from(ValueType::Any))),
    short = "Create a new list with the specified element type.",
    example = "l := ((list string):new)",
)]
struct New {}

fn new(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    match context.this.r#type()? {
        ValueType::List(t) => context.output.send(List::new(*t, []).into()),
        _ => argument_error_legacy("Expected this to be a list type"),
    }
}

#[signature(
    types.list.len,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "The number of values in the list.",
)]
struct Len {}

fn len(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(context.this.list()?.len() as i128))
}

#[signature(
    types.list.empty,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if there are no values in the list.",
)]
struct Empty {}

fn empty(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.list()?.len() == 0))
}

#[signature(
    types.list.push,
    can_block = false,
    output = Known(ValueType::List(Box::from(ValueType::Any))),
    short = "Push elements to the end of the list.",
)]
struct Push {
    #[unnamed()]
    #[description("the values to push")]
    values: Vec<Value>,
}

fn push(mut context: CommandContext) -> CrushResult<()> {
    let l = context.this.list()?;
    let mut cfg: Push = Push::parse(context.remove_arguments(), &context.global_state.printer())?;

    for el in &cfg.values {
        if el.value_type() != l.element_type() && l.element_type() != ValueType::Any {
            return argument_error_legacy(format!("Invalid element type, got {} but expected {}", el.value_type().to_string(), l.element_type().to_string()));
        }
    }
    if !cfg.values.is_empty() {
        l.append(&mut cfg.values)?;
    }
    context.output.send(l.into())
}

#[signature(
    types.list.pop,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Remove the last element from the list.",
)]
struct Pop {
}

fn pop(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.pop().map(|c| o.send(c));
    Ok(())
}

#[signature(
    types.list.peek,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Return the last element from the list without removing it.",
)]
struct Peek {
}

fn peek(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.peek().map(|c| o.send(c));
    Ok(())
}

#[signature(
    types.list.clear,
    can_block = false,
    output = Unknown,
    short = "Remove all values from this list.",
)]
struct Clear {
}

fn clear(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let l = context.this.list()?;
    l.clear();
    context.output.send(l.into())
}

#[signature(
    types.list.__setitem__,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Assign a new value to the element at the specified index.",
)]
struct SetItem {
    #[description("the index of the item to set.")]
    idx: usize,
    #[description("the new value.")]
    value: Value,
}

fn __setitem__(mut context: CommandContext) -> CrushResult<()> {
    let list = context.this.list()?;
    let cfg: SetItem = SetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    list.set(cfg.idx, cfg.value)?;
    context.output.empty()
}

#[signature(
    types.list.remove,
    can_block = false,
    output = Known(ValueType::Any),
    short = "Remove the element at the specified index and return it.",
)]
struct Remove {
    idx: usize,
}

fn remove(mut context: CommandContext) -> CrushResult<()> {
    let list = context.this.list()?;
    let cfg: Remove = Remove::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(list.remove(cfg.idx)?)
}

#[signature(
    types.list.insert,
    can_block = false,
    output = Unknown,
    short = "Insert a new element at the specified index. Following values will be shifted forward.",
)]
struct Insert {
    idx: usize,
    value: Value,
}

fn insert(mut context: CommandContext) -> CrushResult<()> {
    let list = context.this.list()?;
    let cfg: Insert = Insert::parse(context.remove_arguments(), &context.global_state.printer())?;
    list.insert(cfg.idx, cfg.value)
}

#[signature(
    types.list.truncate,
    can_block = false,
    output = Unknown,
    short = "Remove all elements past the specified index.",
)]
struct Truncate {
    idx: Option<usize>,
}

fn truncate(mut context: CommandContext) -> CrushResult<()> {
    let list = context.this.list()?;
    let cfg: Truncate = Truncate::parse(context.remove_arguments(), &context.global_state.printer())?;
    list.truncate(cfg.idx.unwrap_or_default());
    Ok(())
}

#[signature(
    types.list.clone,
    can_block = false,
    output = Known(ValueType::List(Box::from(ValueType::Any))),
    short = "Create a duplicate of the list.",
)]
struct CloneCmd {}

fn clone(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(context.this.list()?.copy().into())
}


#[signature(
    types.list.__getitem__,
    can_block = false,
    output = Known(ValueType::Any),
    short = "Return the value at the specified index of the list.",
)]
struct GetItem {
    #[description("the index of the item to get.")]
    idx: usize,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let list = context.this.list()?;
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(list.get(cfg.idx)?)
}

#[signature(
    types.list.slice,
    can_block = false,
    output=Unknown,
    short = "Extract a slice from this list.",
)]
struct Slice {
    #[description("Starting index (inclusive). If unspecified, from start of list.")]
    #[default(0usize)]
    from: usize,
    #[description("ending index (exclusive). If unspecified, to end of list.")]
    to: Option<usize>,
}

fn slice(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Slice = Slice::parse(context.remove_arguments(), &context.global_state.printer())?;
    let s = context.this.list()?;
    let to = cfg.to.unwrap_or(s.len());

    if to < cfg.from {
        return argument_error_legacy("From larger than to");
    }
    if to > s.len() {
        return argument_error_legacy("Substring beyond end of string");
    }
    context
        .output
        .send(s.slice(cfg.from,to)?.into())
}
