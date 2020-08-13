use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, data_error, mandate, CrushResult};
use crate::lang::execution_context::{ArgumentVector, ExecutionContext, This};
use crate::lang::value::Value;
use crate::lang::{command::Command, list::List, value::ValueType};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "list", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "list"];
        res.declare(
            full("len"),
            len,
            false,
            "list:len",
            "The number of elements in the list",
            None,
            Known(ValueType::Integer),
        );
        res.declare(
            full("empty"),
            empty,
            false,
            "list:empty",
            "True if there are no elements in the list",
            None,
            Known(ValueType::Bool),
        );
        res.declare(
            full("push"),
            push,
            false,
            "list:push",
            "Push an element to the end of the list",
            None,
            Unknown,
        );
        res.declare(
            full("pop"),
            pop,
            false,
            "list:pop",
            "Remove the last element from the list",
            None,
            Unknown,
        );
        res.declare(
            full("peek"),
            peek,
            false,
            "list:peek",
            "Return the last element from the list",
            None,
            Unknown,
        );
        res.declare(
            full("clear"),
            clear,
            false,
            "list:clear",
            "Remove all elments from the list",
            None,
            Unknown,
        );
        res.declare(
            full("__setitem__"),
            setitem,
            false,
            "list[idx:integer] = value:any",
            "Assign a new value to the element at the specified index",
            None,
            Known(ValueType::Empty),
        );
        res.declare(
            full("remove"),
            remove,
            false,
            "list:remove idx:integer",
            "Remove the element at the specified index",
            None,
            Unknown,
        );
        res.declare(
            full("insert"),
            insert,
            false,
            "list:insert idx:integer value:any",
            "Insert a new element at the specified index",
            None,
            Unknown,
        );
        res.declare(
            full("truncate"),
            truncate,
            false,
            "list:truncate idx:integer",
            "Remove all elements past the specified index",
            None,
            Unknown,
        );
        res.declare(
            full("clone"),
            clone,
            false,
            "list:clone",
            "Create a duplicate of the list",
            None,
            Unknown,
        );
        res.declare(
            full("of"),
            of,
            true,
            "list:of element:any...",
            "Create a new list containing the supplied elements",
            Some("    If no elements are supplied as arguments, input must be a stream with\n    exactly one column."),
            Unknown,
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
        );
        res.declare(
            full("__call_type__"),
            call_type,
            false,
            "list element_type:type",
            "Return a list type for the specified element type",
            Some(
                r#"    Example:

    # This command returns the type 'list of integers':
    list integer"#,
            ),
            Known(ValueType::Type),
        );
        res.declare(
            full("__getitem__"),
            getitem,
            true,
            "name[idx:index]",
            "Return a file or subdirectory in the specified base directory",
            None,
            Unknown,
        );
        let _ = Repeat::declare_method(&mut res, &path); // TODO: why unused?

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

fn repeat(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Repeat = Repeat::parse(context.arguments, &context.printer)?;
    let mut l = Vec::with_capacity(cfg.times as usize);
    for _i in 0..cfg.times {
        l.push(cfg.item.clone());
    }
    context
        .output
        .send(Value::List(List::new(cfg.item.value_type(), l)))
}

fn call_type(mut context: ExecutionContext) -> CrushResult<()> {
    match context.this.r#type()? {
        ValueType::List(c) => match *c {
            ValueType::Empty => {
                context.arguments.check_len(1)?;
                context.output.send(Value::Type(ValueType::List(Box::new(
                    context.arguments.r#type(0)?,
                ))))
            }
            c => {
                if context.arguments.is_empty() {
                    context
                        .output
                        .send(Value::Type(ValueType::List(Box::from(c))))
                } else {
                    argument_error(
                        format!(
                            "Tried to set subtype on a list that already has the subtype {}",
                            c.to_string()
                        )
                        .as_str(),
                    )
                }
            }
        },
        _ => argument_error("Invalid this, expected type list"),
    }
}

fn of(mut context: ExecutionContext) -> CrushResult<()> {
    match context.arguments.len() {
        0 => {
            let mut lst = Vec::new();
            let mut input = mandate(context.input.recv()?.stream(), "Expected a stream")?;
            if input.types().len() != 1 {
                return data_error("Expected input with exactly one column");
            }
            while let Ok(row) = input.read() {
                lst.push(row.into_vec().remove(0));
            }
            if lst.is_empty() {
                return data_error("Empty stream!");
            }
            context
                .output
                .send(Value::List(List::new_without_type(lst)))
        }
        _ => {
            let lst =
                List::new_without_type(context.arguments.drain(..).map(|a| a.value).collect());
            context.output.send(Value::List(lst))
        }
    }
}

fn new(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    match context.this.r#type()? {
        ValueType::List(t) => context.output.send(Value::List(List::new(*t, vec![]))),
        _ => argument_error("Expected this to be a list type"),
    }
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(context.this.list()?.len() as i128))
}

fn empty(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.list()?.len() == 0))
}

fn push(mut context: ExecutionContext) -> CrushResult<()> {
    let l = context.this.list()?;
    let mut new_elements: Vec<Value> = Vec::new();
    for el in context.arguments.drain(..) {
        if el.value.value_type() == l.element_type() || l.element_type() == ValueType::Any {
            new_elements.push(el.value)
        } else {
            return argument_error("Invalid element type");
        }
    }
    if !new_elements.is_empty() {
        l.append(&mut new_elements)?;
    }
    context.output.send(Value::List(l))
}

fn pop(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.pop().map(|c| o.send(c));
    Ok(())
}

fn peek(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let o = context.output;
    context.this.list()?.peek().map(|c| o.send(c));
    Ok(())
}

fn clear(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let l = context.this.list()?;
    l.clear();
    context.output.send(Value::List(l))
}

fn setitem(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let list = context.this.list()?;
    let key = context.arguments.integer(0)?;
    let value = context.arguments.value(1)?;
    list.set(key as usize, value)?;
    context.output.empty()
}

fn remove(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    list.remove(idx as usize)
}

fn insert(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    let value = context.arguments.value(1)?;
    list.insert(idx as usize, value)
}

fn truncate(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    list.truncate(idx as usize);
    Ok(())
}

fn clone(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::List(context.this.list()?.copy()))
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let list = context.this.list()?;
    let idx = context.arguments.integer(0)?;
    context.output.send(list.get(idx as usize)?)
}
