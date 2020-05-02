use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, ExecutionContext, This};
use crate::lang::value::Value;
use crate::lang::{command::CrushCommand, list::List, value::ValueType};
use lazy_static::lazy_static;
use std::collections::HashMap;
use std::collections::HashSet;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "list", name]
}

lazy_static! {
    pub static ref METHODS: HashMap<String, Box<dyn CrushCommand + Sync + Send>> = {
        let mut res: HashMap<String, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.declare(
            full("len"),
            len,
            false,
            "list:len",
            "The number of elements in the list",
            None,
        );
        res.declare(
            full("empty"),
            empty,
            false,
            "list:empty",
            "True if there are no elements in the list",
            None,
        );
        res.declare(
            full("push"),
            push,
            false,
            "list:push",
            "Push an element to the end of the list",
            None,
        );
        res.declare(
            full("pop"),
            pop,
            false,
            "list:pop",
            "Remove the last element from the list",
            None,
        );
        res.declare(
            full("peek"),
            peek,
            false,
            "list:peek",
            "Return the last element from the list",
            None,
        );
        res.declare(
            full("clear"),
            clear,
            false,
            "list:clear",
            "Remove all elments from the list",
            None,
        );
        res.declare(
            full("__setitem__"),
            setitem,
            false,
            "list[idx:integer] = value:any",
            "Assign a new value to the element at the specified index",
            None,
        );
        res.declare(
            full("remove"),
            remove,
            false,
            "list:remove idx:integer",
            "Remove the element at the specified index",
            None,
        );
        res.declare(
            full("insert"),
            insert,
            false,
            "list:insert idx:integer value:any",
            "Insert a new element at the specified index",
            None,
        );
        res.declare(
            full("truncate"),
            truncate,
            false,
            "list:truncate idx:integer",
            "Remove all elements past the specified index",
            None,
        );
        res.declare(
            full("clone"),
            clone,
            false,
            "list:clone",
            "Create a duplicate of the list",
            None,
        );
        res.declare(
            full("of"),
            of,
            false,
            "list:of element:any...",
            "Create a new list containing the supplied elements",
            None,
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
        );
        res.declare(
            full("__getitem__"),
            getitem,
            true,
            "name[idx:index]",
            "Return a file or subdirectory in the specified base directory",
            None,
        );
        res
    };
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
    context.arguments.check_len_min(1)?;

    let types = context
        .arguments
        .iter()
        .map(|a| a.value.value_type())
        .collect::<HashSet<ValueType>>();
    let lst = List::new(
        if types.len() == 1 {
            context.arguments[0].value.value_type()
        } else {
            ValueType::Any
        },
        context.arguments.drain(..).map(|a| a.value).collect(),
    );
    context.output.send(Value::List(lst))
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
    context.this.list()?.clear();
    Ok(())
}

fn setitem(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let list = context.this.list()?;
    let key = context.arguments.integer(0)?;
    let value = context.arguments.value(1)?;
    list.set(key as usize, value)
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
