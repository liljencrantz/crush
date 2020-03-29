use crate::lang::errors::{CrushResult, mandate};
use crate::lang::{execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("__getitem__"), CrushCommand::command(
        getitem, false,
            "scope[name:string]",
            "Return the specified member",
            None));
        res
    };
}

fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    let val = context.this.scope()?;
    context.arguments.check_len(1)?;
    let name = context.arguments.string(0)?;
    context.output.send(mandate(val.get(name.as_ref()), "Unknown member")?)
}
