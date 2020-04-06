use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value};
use crate::lang::scope::Scope;
use std::cmp::Ordering;

macro_rules! cmp {
    ($name:ident, $op:expr) => {
pub fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let l = context.arguments.value(0)?;
    let r = context.arguments.value(1)?;
    match l.partial_cmp(&r) {
        Some(ordering) => context.output.send(Value::Bool($op(ordering))),
        None => return argument_error(
            format!(
                "Values of type {} and {} can't be compared with each other",
                l.value_type().to_string(),
                r.value_type().to_string(),
            ).as_str()),
    }
}
    }
}

cmp!(gt, |o| o == Ordering::Greater);
cmp!(lt, |o| o == Ordering::Less);
cmp!(gte, |o| o != Ordering::Less);
cmp!(lte, |o| o != Ordering::Greater);

pub fn eq(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let l = context.arguments.value(0)?;
    let r = context.arguments.value(1)?;
    context.output.send(Value::Bool(l.eq(&r)))
}

pub fn neq(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let l = context.arguments.value(0)?;
    let r = context.arguments.value(1)?;
    context.output.send(Value::Bool(!l.eq(&r)))
}

pub fn not(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    context.output.send(Value::Bool(!context.arguments.bool(0)?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("comp")?;
    env.declare_command("gt", gt, false, "any > any", "True if left side is greater than right side", None)?;
    env.declare_command("gte", gte, false, "any >= any", "True if left side is greater than or equal to right side", None)?;
    env.declare_command("lt", lt, false, "any < any", "True if left side is less than right side", None)?;
    env.declare_command("lte", lte, false, "any <= any", "True if left side is less than or equal to right side", None)?;
    env.declare_command("eq", eq, false, "any == any", "True if left side is equal to right side", None)?;
    env.declare_command("neq", neq, false, "any != any", "True if left side is not equal to right side", None)?;
    env.declare_command("not", not, false, "not boolean", "Negates a boolean", None)?;
    env.readonly();
    Ok(())
}
