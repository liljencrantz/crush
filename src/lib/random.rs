use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value};
use crate::lang::scope::Scope;
use crate::lang::execution_context::ArgumentVector;
use crate::lang::errors::argument_error;
use lazy_static::lazy_static;
use rand::prelude::*;
use signature::signature;
use crate::lang::argument::ArgumentHandler;
/*
lazy_static! {
    pub static ref RNG: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        res
    };
}
*/
#[signature(
    random,
    can_block = false,
    short = "generate a random number between 0 and 1")]
struct Random {}

fn random(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Float(rand::random()))?;
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_lazy_namespace(
        "random",
        Box::new(move |env| {
            Random::declare(env)?;
            Ok(())
        }))?;
    Ok(())
}
