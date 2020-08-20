use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::scope::Scope;
use crate::lang::stream::{channels, empty_channel};
use crate::lang::value::Value;

pub fn and(mut context: CommandContext) -> CrushResult<()> {
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
                let cc = CommandContext {
                    input: empty_channel(),
                    output: sender,
                    arguments: vec![],
                    scope: context.scope.clone(),
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

pub fn or(mut context: CommandContext) -> CrushResult<()> {
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
                let cc = CommandContext {
                    input: empty_channel(),
                    output: sender,
                    arguments: vec![],
                    scope: context.scope.clone(),
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
    root.create_namespace(
        "cond",
        Box::new(|env| {
            env.declare_condition_command("and",
                                          and,
                                          "and condition:(bool|command)... -> boolean",
                                          "True if all arguments are true",
                                          Some(r#"    Every argument to and must be either a boolean or a command that returns a boolean.
    The and command will check all arguments in order, and if any of them are false, and
    will return false. If all conditions are true, and returns true.

    Do note that and is a short circuiting command, meaning that if one of the conditions
    is found to be false, and will not evaluate any remaining closures."#))?;

            env.declare_condition_command("or",
                                          or,
                                          "or condition:(bool|command)... -> boolean",
                                          "True if any argument is true",
                                          Some(r#"    Every argument to or must be either a boolean or a command that returns a boolean.
    The or command will check all arguments in order, and if any of them are true, or
    will return true. If all conditions are false, or returns false.

    Do note that or is a short circuiting command, meaning that if one of the conditions
    is found to be true, or will not evaluate any remaining closures."#))?;

            Ok(())
        }))?;
    Ok(())
}
