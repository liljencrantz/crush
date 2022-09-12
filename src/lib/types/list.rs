use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error_legacy, data_error, mandate, CrushResult, argument_error};
use crate::lang::state::contexts::{ArgumentVector, CommandContext, This};
use crate::lang::value::Value;
use crate::lang::{command::Command, data::list::List, value::ValueType};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::data::table::ColumnVec;
use crate::util::replace::Replace;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "list", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "list"];
        Len::declare_method(&mut res, &path);
        Empty::declare_method(&mut res, &path);
        Clear::declare_method(&mut res, &path);
        res.declare(
            full("push"),
            push,
            false,
            "list:push",
            "Push an element to the end of the list",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("pop"),
            pop,
            false,
            "list:pop",
            "Remove the last element from the list",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("peek"),
            peek,
            false,
            "list:peek",
            "Return the last element from the list",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("__setitem__"),
            setitem,
            false,
            "list[idx:integer] = value:any",
            "Assign a new value to the element at the specified index",
            None,
            Known(ValueType::Empty),
            vec![],
        );
        res.declare(
            full("remove"),
            remove,
            false,
            "list:remove idx:integer",
            "Remove the element at the specified index",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("insert"),
            insert,
            false,
            "list:insert idx:integer value:any",
            "Insert a new element at the specified index",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("truncate"),
            truncate,
            false,
            "list:truncate idx:integer",
            "Remove all elements past the specified index",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("clone"),
            clone,
            false,
            "list:clone",
            "Create a duplicate of the list",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("of"),
            of,
            true,
            "list:of element:any...",
            "Create a new list containing the supplied elements",
            None,
            Unknown,
            vec![],
        );
        res.declare(
            full("collect"),
            collect,
            true,
            "list:collect [column]",
            "Create a new list by reading a column from the input",
            Some("    If no elements are supplied as arguments, input must be a stream with\n    exactly one column."),
            Unknown,
            vec![],
        );
        res.declare(
            full("new"),
            new,
            false,
            "list:new",
            "Create a new list with the specified element type",
            Some(
                r#"    Example:

    l := ((list string):new)"#,
            ),
            Unknown,
            vec![],
        );
        res.declare(
            full("__getitem__"),
            getitem,
            true,
            "name[idx:index]",
            "Return a file or subdirectory in the specified base directory",
            None,
            Unknown,
            vec![],
        );
        Repeat::declare_method(&mut res, &path);
        Call::declare_method(&mut res, &path);

        res
    };
}

#[signature(
    repeat,
    can_block = false,
    short = "Create a list containing the same value multiple times"
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
        .send(Value::List(List::new(cfg.item.value_type(), l)))
}

#[signature(
__call__,
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

fn of(mut context: CommandContext) -> CrushResult<()> {
    match context.arguments.len() {
        0 => argument_error_legacy("Expected at least one argument"),
        _ => context.output.send(
            Value::List(List::new_without_type(
                context.arguments
                    .drain(..)
                    .map(|a| a.value)
                    .collect()
            ))
        ),

    }
}

fn collect(mut context: CommandContext) -> CrushResult<()> {
    let mut input = mandate(context.input.recv()?.stream()?, "Expected a stream")?;
    let input_type = input.types().to_vec();
    let mut lst = Vec::new();
    match context.arguments.len() {
        0 => {
            if input_type.len() != 1 {
                return data_error("Expected input with exactly one column");
            }
            while let Ok(row) = input.read() {
                lst.push(Vec::from(row).remove(0));
            }
            context
                .output
                .send(Value::List(List::new(input_type[0].cell_type.clone(), lst)))
        }
        1 => {
            match &context.arguments[0].value {
                Value::String(s) => {
                    match input_type.as_slice().find(s) {
                        Ok(idx) => {
                            while let Ok(row) = input.read() {
                                lst.push(Vec::from(row).replace(idx, Value::Empty()));
                            }
                            context
                                .output
                                .send(Value::List(List::new(input_type[idx].cell_type.clone(), lst)))
                        }
                        _ => argument_error("Column not found", context.arguments[0].location)
                    }
                }
                _ => argument_error("Expected argument of type symbol", context.arguments[0].location),
            }
        }
        _ => argument_error("Expected a single argument", context.arguments[0].location),
    }
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    match context.this.r#type()? {
        ValueType::List(t) => context.output.send(Value::List(List::new(*t, vec![]))),
        _ => argument_error_legacy("Expected this to be a list type"),
    }
}

#[signature(
len,
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
empty,
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

fn push(mut context: CommandContext) -> CrushResult<()> {
    let l = context.this.list()?;
    let mut new_elements: Vec<Value> = Vec::new();
    for el in context.arguments.drain(..) {
        if el.value.value_type() == l.element_type() || l.element_type() == ValueType::Any {
            new_elements.push(el.value)
        } else {
            return argument_error_legacy("Invalid element type");
        }
    }
    if !new_elements.is_empty() {
        l.append(&mut new_elements)?;
    }
    context.output.send(Value::List(l))
}

fn pop(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.pop().map(|c| o.send(c));
    Ok(())
}

fn peek(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.peek().map(|c| o.send(c));
    Ok(())
}

#[signature(
clear,
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
    context.output.send(Value::List(l))
}

fn setitem(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let list = context.this.list()?;
    let key = context.arguments.integer(0)?;
    let value = context.arguments.value(1)?;
    list.set(key as usize, value)?;
    context.output.empty()
}

fn remove(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    list.remove(idx as usize)
}

fn insert(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    let value = context.arguments.value(1)?;
    list.insert(idx as usize, value)
}

fn truncate(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    list.truncate(idx as usize);
    Ok(())
}

fn clone(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::List(context.this.list()?.copy()))
}

fn getitem(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    context.output.send(list.get(idx as usize)?)
}
