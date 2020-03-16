use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{command::SimpleCommand, value::Value};
use crate::lang::scope::Scope;
use std::cmp::Ordering;
use crate::lang::command::ConditionCommand;
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
            Value::Closure(c) => {
                let (sender, receiver) = channels();
                let cc = ExecutionContext {
                    input: empty_channel(),
                    output: sender,
                    arguments: vec![],
                    env: context.env.clone(),
                    this: None,
                    printer: context.printer.clone(),
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

            Value::Closure(c) => {
                let (sender, receiver) = channels();
                let cc = ExecutionContext {
                    input: empty_channel(),
                    output: sender,
                    arguments: vec![],
                    env: context.env.clone(),
                    this: None,
                    printer: context.printer.clone(),
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
    root.r#use(&env);
    env.declare("and", Value::ConditionCommand(ConditionCommand::new(and)))?;
    env.declare("or", Value::ConditionCommand(ConditionCommand::new(or)))?;
    env.readonly();
    Ok(())
}
