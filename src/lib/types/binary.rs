use crate::lang::errors::{CrushResult, mandate};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "binary", name]
}

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.declare(full("len"),
            len, false,
            "binary:len",
            "The number of bytes in the binary",
            None);
        res.declare(full("__getitem__"),
            getitem, false,
            "binary[idx:integer]", "Returns the byte at the specified offset", None);
        res
    };
}

fn len(context: ExecutionContext) -> CrushResult<()> {
    let val = context.this.binary()?;
    context.output.send(Value::Integer(val.len() as i128))
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    let val = context.this.binary()?;
    context.arguments.check_len(1)?;
    let idx = context.arguments.integer(0)?;
    context.output.send(Value::Integer(mandate(val.get(idx as usize), "Index out of bounds")?.clone() as i128))
}
