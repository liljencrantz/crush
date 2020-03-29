use crate::lang::command::CrushCommand;
use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{CrushResult};
use crate::lang::{value::Value};
use crate::lang::scope::Scope;
use crate::lang::execution_context::ArgumentVector;

macro_rules! math_fun {
    ($name:ident, $op:expr) => {
fn $name(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let x: f64 = context.arguments.float(0)?;
    context.output.send(Value::Float($op(x)))
}
    }
}

math_fun!(sin, |x:f64| x.sin());
math_fun!(cos, |x:f64| x.cos());
math_fun!(tan, |x:f64| x.tan());
math_fun!(sqrt, |x:f64| x.sqrt());
math_fun!(asin, |x:f64| x.asin());
math_fun!(acos, |x:f64| x.acos());
math_fun!(atan, |x:f64| x.atan());

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("math")?;
    env.declare("sin", Value::Command(CrushCommand::command(
        sin, false,
        "sin angle:float",
        "The sine of the specified angle",
        None)))?;
    env.declare("cos", Value::Command(CrushCommand::command(
        cos, false,
        "cos angle:float",
        "The cosine of the specified angle",
        None)))?;
    env.declare("tan", Value::Command(CrushCommand::command(
        tan, false,
        "tan angle:float",
        "The tangent of the specified angle",
        None)))?;
    env.declare("sqrt", Value::Command(CrushCommand::command(
        sqrt, false,
        "sqrt angle:float",
        "The square root of the specified angle",
        None)))?;
    env.declare("asin", Value::Command(CrushCommand::command(
        asin, false,
        "asin arc:float",
        "The inverse sine of the specified arc",
        None)))?;
    env.declare("acos", Value::Command(CrushCommand::command(
        acos, false,
        "acos arc:float",
        "The inverse cosine of the specified arc",
        None)))?;
    env.declare("atan", Value::Command(CrushCommand::command(
        atan, false,
        "atan arc:float",
        "The inverse tangent of the specified arc",
        None)))?;
    env.readonly();
    Ok(())
}
