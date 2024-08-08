use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::{Scope, ScopeType};
use crate::lang::pipe::pipe;
use crate::lang::value::Value;

pub fn and(mut context: CommandContext) -> CrushResult<()> {
    let mut res = true;
    for arg in context.remove_arguments().drain(..) {
        match arg.value {
            Value::Bool(b) => {
                if !b {
                    res = false;
                    break;
                }
            }
            Value::Command(c) => {
                let (sender, receiver) = pipe();
                let env = context.scope.create_child(&context.scope, ScopeType::Conditional);
                let cc = context.empty().with_output(sender).with_scope(env);
                c.eval(cc)?;
                if context.scope.is_stopped() {
                    return Ok(());
                }
                match receiver.recv()? {
                    Value::Bool(b) => {
                        if !b {
                            res = false;
                            break;
                        }
                    }
                    v => return argument_error_legacy(format!("Expected boolean values, got a value of type {}", v.value_type().to_string())),
                }
            }
            _ => return argument_error_legacy("Expected boolean values"),
        }
    }
    context.output.send(Value::Bool(res))
}

pub fn or(mut context: CommandContext) -> CrushResult<()> {
    let mut res = false;
    for arg in context.remove_arguments().drain(..) {
        match arg.value {
            Value::Bool(b) => {
                if b {
                    res = true;
                    break;
                }
            }

            Value::Command(c) => {
                let (sender, receiver) = pipe();
                let env = context.scope.create_child(&context.scope, ScopeType::Conditional);
                let cc = context.empty().with_output(sender).with_scope(env);
                c.eval(cc)?;
                if context.scope.is_stopped() {
                    return Ok(());
                }
                match receiver.recv()? {
                    Value::Bool(b) => {
                        if b {
                            res = true;
                            break;
                        }
                    }
                    _ => return argument_error_legacy("Expected boolean values"),
                }
            }
            _ => return argument_error_legacy("Expected boolean values"),
        }
    }
    context.output.send(Value::Bool(res))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "cond",
        "Logical operators (and and or)",
        Box::new(|env| {
            env.declare_condition_command(
                "__and__",
                and,
                "cond:__and__ condition:(bool|command)... -> boolean",
                "True if all arguments are true",
                Some(r#"    Every argument to and must be either a boolean or a command that returns a boolean.
    The and command will check all arguments in order, and if any of them are false, and
    will return false. If all conditions are true, and returns true.

    Do note that `and` is a short circuiting command, meaning that if one of the conditions
    is found to be false, `and` will not evaluate any remaining closures."#),
                vec![],
            )?;

            env.declare_condition_command(
                "__or__",
                or,
                "cond:__or__ condition:(bool|command)... -> boolean",
                "True if any argument is true",
                Some(r#"    Every argument to or must be either a boolean or a command that returns a boolean.
    The or command will check all arguments in order, and if any of them are true, or
    will return true. If all conditions are false, or returns false.

    Do note that `or` is a short circuiting command, meaning that if one of the conditions
    is found to be true, `or` will not evaluate any remaining closures."#),
                vec![],
            )?;

            Ok(())
        }))?;
    Ok(())
}
