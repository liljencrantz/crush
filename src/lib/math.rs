use crate::lang::command::CrushCommand;
use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::CrushResult;
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
math_fun!(ceil, |x:f64| x.ceil());
math_fun!(floor, |x:f64| x.floor());
math_fun!(ln, |x:f64| x.ln());

fn pow(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let x: f64 = context.arguments.float(0)?;
    let y: f64 = context.arguments.float(1)?;
    context.output.send(Value::Float(x.powf(y)))
}

fn log(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    let x: f64 = context.arguments.float(0)?;
    let y: f64 = context.arguments.float(1)?;
    context.output.send(Value::Float(x.log(y)))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("math")?;
    env.declare("sin", Value::Command(CrushCommand::command(
        sin, false,
        "math:sin angle:float",
        "The sine of the specified angle",
        None)))?;
    env.declare("cos", Value::Command(CrushCommand::command(
        cos, false,
        "math:cos angle:float",
        "The cosine of the specified angle",
        None)))?;
    env.declare("tan", Value::Command(CrushCommand::command(
        tan, false,
        "math:tan angle:float",
        "The tangent of the specified angle",
        None)))?;
    env.declare("sqrt", Value::Command(CrushCommand::command(
        sqrt, false,
        "math:sqrt angle:float",
        "The square root of the specified angle",
        None)))?;
    env.declare("asin", Value::Command(CrushCommand::command(
        asin, false,
        "math:asin arc:float",
        "The inverse sine of the specified arc",
        None)))?;
    env.declare("acos", Value::Command(CrushCommand::command(
        acos, false,
        "math:acos arc:float",
        "The inverse cosine of the specified arc",
        None)))?;
    env.declare("atan", Value::Command(CrushCommand::command(
        atan, false,
        "math:atan arc:float",
        "The inverse tangent of the specified arc",
        None)))?;
    env.declare("pow", Value::Command(CrushCommand::command(
        pow, false,
        "math:pow number:float n:float",
        "Raise the number to n",
        None)))?;
    env.declare("log", Value::Command(CrushCommand::command(
        log, false,
        "math:log number:float base:float",
        "The logarithm of number in the specified base",
        None)))?;
    env.declare("ln", Value::Command(CrushCommand::command(
        ln, false,
        "math:ln number:float",
        "The natural logarithm of number",
        None)))?;
    env.declare("floor", Value::Command(CrushCommand::command(
        floor, false,
        "math:floor number:float",
        "The largest integer smaller than number",
        None)))?;
    env.declare("ceil", Value::Command(CrushCommand::command(
        ceil, false,
        "math:ceil number:float",
        "The smallest integer larger than number",
        None)))?;

    env.readonly();
    Ok(())
}
