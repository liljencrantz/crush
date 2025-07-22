use crate::lang::command::CrushCommand;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::{Scope, ScopeType};
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
                let env = context
                    .scope
                    .create_child(&context.scope, ScopeType::Conditional);
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
                    v => {
                        return argument_error(format!(
                            "Expected boolean values, got a value of type {}",
                            v.value_type().to_string()
                        ), &context.source);
                    }
                }
            }
            _ => return argument_error("Expected boolean values", &context.source),
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
                let env = context
                    .scope
                    .create_child(&context.scope, ScopeType::Conditional);
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
                    _ => return argument_error("Expected boolean values", &context.source),
                }
            }
            _ => return argument_error("Expected boolean values", &context.source),
        }
    }
    context.output.send(Value::Bool(res))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "cond",
        "Logical operators (`and` and `or`)",
        Box::new(|env| {
            env.declare(
                "and",
                Value::Command(
                    <dyn CrushCommand>::condition(
                        and,
                        vec!["cond".to_string(), "and".to_string()],
                        "and condition:(bool|command)... -> boolean",
                        "True if all arguments are true",
                        Some("Every argument to and must be either a boolean or a command that returns a boolean.
The and command will check all arguments in order, and if any of them are false, and
will return false. If all conditions are true, and returns true.

Do note that `and` is a short circuiting command, meaning that if one of the conditions
is found to be false, `and` will not evaluate any remaining closures.

In expression mode, this method can be used via the the `and` operator.

# Examples
```
# true, if $file exists and is a symlink
and $($file:exists) {$(stat $file)[0]:is_symlink}

# true, if $file exists and is a symlink
($file.exists() and {stat($file)[0].is_symlink})
```
"),
                        vec![],
                    )))?;

            env.declare(
                "or",
                Value::Command(
                    <dyn CrushCommand>::condition(
                        or,
                        vec!["cond".to_string(), "or".to_string()],
                        "or condition:(bool|command)... -> boolean",
                        "True if any argument is true",
                        Some("Every argument to or must be either a `boolean` or a command that returns a `boolean`.
The or command will check all arguments in order, and if any of them are `true`, or
will return `true`. If all conditions are `false`, or returns `false`.

Do note that `or` is a short circuiting command, meaning that if one of the conditions
is found to be true, `or` will not evaluate any remaining closures.

In expression mode, this method can be used via the the `or` operator.

# Examples
```
$stat_out := $(stat $file)[0]

# true, if $file is either a symlink or a directory
or $stat_out:is_symlink $stat_out:is_dir

# true, if $file is either a symlink or a directory
($stat_out.is_symlink or $stat_out.is_dir)
```
"),
                        vec![],
                    )))?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
