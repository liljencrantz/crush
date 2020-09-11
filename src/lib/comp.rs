use crate::lang::command::OutputType::Known;
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, CommandContext};
use crate::lang::data::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use std::cmp::Ordering;

macro_rules! cmp {
    ($name:ident, $op:expr) => {
        pub fn $name(mut context: CommandContext) -> CrushResult<()> {
            context.arguments.check_len(2)?;
            let l = context.arguments.value(0)?;
            let r = context.arguments.value(1)?;
            match l.partial_cmp(&r) {
                Some(ordering) => context.output.send(Value::Bool($op(ordering))),
                None => {
                    return argument_error(
                        format!(
                            "Values of type {} and {} can't be compared with each other",
                            l.value_type().to_string(),
                            r.value_type().to_string(),
                        )
                        .as_str(),
                    )
                }
            }
        }
    };
}

cmp!(gt, |o| o == Ordering::Greater);
cmp!(lt, |o| o == Ordering::Less);
cmp!(gte, |o| o != Ordering::Less);
cmp!(lte, |o| o != Ordering::Greater);

pub fn eq(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let l = context.arguments.value(0)?;
    let r = context.arguments.value(1)?;
    context.output.send(Value::Bool(l.eq(&r)))
}

pub fn neq(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let l = context.arguments.value(0)?;
    let r = context.arguments.value(1)?;
    context.output.send(Value::Bool(!l.eq(&r)))
}

pub fn not(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context
        .output
        .send(Value::Bool(!context.arguments.bool(0)?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "comp",
        Box::new(|env| {
            env.declare_command(
                "gt",
                gt,
                false,
                "any > any",
                "True if left side is greater than right side",
                None,
                Known(ValueType::Bool),
                vec![],
            )?;
            env.declare_command(
                "gte",
                gte,
                false,
                "any >= any",
                "True if left side is greater than or equal to right side",
                None,
                Known(ValueType::Bool),
                vec![],
            )?;
            env.declare_command(
                "lt",
                lt,
                false,
                "any < any",
                "True if left side is less than right side",
                None,
                Known(ValueType::Bool),
                vec![],
            )?;
            env.declare_command(
                "lte",
                lte,
                false,
                "any <= any",
                "True if left side is less than or equal to right side",
                None,
                Known(ValueType::Bool),
                vec![],
            )?;
            env.declare_command(
                "eq",
                eq,
                false,
                "any == any",
                "True if left side is equal to right side",
                None,
                Known(ValueType::Bool),
                vec![],
            )?;
            env.declare_command(
                "neq",
                neq,
                false,
                "any != any",
                "True if left side is not equal to right side",
                None,
                Known(ValueType::Bool),
                vec![],
            )?;
            env.declare_command(
                "__not__",
                not,
                false,
                "not boolean",
                "Negates a boolean value",
                None,
                Known(ValueType::Bool),
                vec![],
            )?;
            Ok(())
        }),
    )?;
    Ok(())
}
