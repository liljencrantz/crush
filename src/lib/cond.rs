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
    let and_help = r#"and condition:(bool|command)... -> boolean
    Every argument to and must be either a boolean or a command that returns a boolean.
    The and command will check all arguments in order, and if any of them are false, and
    will return false. If all conditions are true, and returns true.

    Do note that and is a short circuiting command, meaning that if one of the conditions
    is found to be false, and will not evaluate any remaining closures."#;

    let or_help = r#"or condition:(bool|command)... -> boolean
    Every argument to or must be either a boolean or a command that returns a boolean.
    The or command will check all arguments in order, and if any of them are true, or
    will return true. If all conditions are false, or returns false.

    Do note that or is a short circuiting command, meaning that if one of the conditions
    is found to be true, or will not evaluate any remaining closures."#;

    env.declare("__and__", Value::Command(CrushCommand::condition(and, and_help)))?;
    env.declare("__or__", Value::Command(CrushCommand::condition(or, or_help)))?;
    env.readonly();
    Ok(())
}
