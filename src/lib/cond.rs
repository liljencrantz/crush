use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value};
use crate::lang::scope::Scope;
use crate::lang::stream::{empty_channel, channels};

pub fn and(mut context: ExecutionContext) -> CrushResult<()> {
    let mut res = true;
    for arg in context.arguments.drain(..) {
        match arg.value {
            Value::Bool(b) => {
                if !b {
                    res = false;
                    break;
                }
            }
            Value::Command(c) => {
                let (sender, receiver) = channels();
                let cc = ExecutionContext {
                    input: empty_channel(),
                    output: sender,
                    arguments: vec![],
                    env: context.env.clone(),
                    this: None,
                };
                c.invoke(cc)?;
                match receiver.recv()? {
                    Value::Bool(b) => {
                        if !b {
                            res = false;
                            break;
                        }
                    }
                    _ => return argument_error("Expected boolean values"),
                }
            }
            _ => return argument_error("Expected boolean values"),
        }
    }
    context.output.send(Value::Bool(res))
}

pub fn or(mut context: ExecutionContext) -> CrushResult<()> {
    let mut res = false;
    for arg in context.arguments.drain(..) {
        match arg.value {
            Value::Bool(b) => {
                if b {
                    res = true;
                    break;
                }
            }

            Value::Command(c) => {
                let (sender, receiver) = channels();
                let cc = ExecutionContext {
                    input: empty_channel(),
                    output: sender,
                    arguments: vec![],
                    env: context.env.clone(),
                    this: None,
                };
                c.invoke(cc)?;
                match receiver.recv()? {
                    Value::Bool(b) => {
                        if b {
                            res = true;
                            break;
                        }
                    }
                    _ => return argument_error("Expected boolean values"),
                }
            }
            _ => return argument_error("Expected boolean values"),
        }
    }
    context.output.send(Value::Bool(res))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("cond")?;
    env.declare("and", Value::Command(CrushCommand::condition(and)))?;
    env.declare("or", Value::Command(CrushCommand::condition(or)))?;
    env.readonly();
    Ok(())
}
