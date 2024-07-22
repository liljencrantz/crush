use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use signature::signature;
use crate::lang::signature::number::Number;

macro_rules! math_fun {
    ($name:ident, $Signature: ident, $op:expr) => {
        fn $name(context: CommandContext) -> CrushResult<()> {
            let cfg: $Signature = $Signature::parse(context.arguments, &context.global_state.printer())?;
            context.output.send(Value::Float($op(cfg.number.as_float())))
        }
    };
}

#[signature(
    math.sin,
    output = Known(ValueType::Float),
    short = "The sine of number.")]
pub struct Sin {
    number: Number,
}
math_fun!(sin, Sin, |x: f64| x.sin());

#[signature(
    math.cos,
    output = Known(ValueType::Float),
    short = "The cosine of number.")]
pub struct Cos {
    number: Number,
}
math_fun!(cos, Cos, |x: f64| x.cos());

#[signature(
    math.tan,
    output = Known(ValueType::Float),
    short = "The tangent of number.")]
pub struct Tan {
    number: Number,
}
math_fun!(tan, Tan, |x: f64| x.tan());

#[signature(
    math.sqrt,
    output = Known(ValueType::Float),
    short = "The square root of number.")]
pub struct Sqrt {
    number: Number,
}
math_fun!(sqrt, Sqrt, |x: f64| x.sqrt());

#[signature(
    math.asin,
    output = Known(ValueType::Float),
    short = "The arc sine of number.")]
pub struct ASin {
    number: Number,
}
math_fun!(asin, ASin, |x: f64| x.asin());

#[signature(
    math.acos,
    output = Known(ValueType::Float),
    short = "The arc cosine of number.")]
pub struct ACos {
    number: Number,
}
math_fun!(acos, ACos, |x: f64| x.acos());

#[signature(
    math.atan,
    output = Known(ValueType::Float),
    short = "The arc tangent of number.")]
pub struct ATan {
    number: Number,
}
math_fun!(atan, ATan, |x: f64| x.atan());

#[signature(
    math.ceil,
    output = Known(ValueType::Float),
    short = "The smallest integer larger than number.")]
pub struct Ceil {
    number: Number,
}
math_fun!(ceil, Ceil, |x: f64| x.ceil());

#[signature(
    math.floor,
    output = Known(ValueType::Float),
    short = "The largest integer smaller than number.")]
pub struct Floor {
    number: Number,
}
math_fun!(floor, Floor, |x: f64| x.floor());

#[signature(
    math.ln,
    output = Known(ValueType::Float),
    short = "The natural logarithm of number.")]
pub struct Ln {
    number: Number,
}
math_fun!(ln, Ln, |x: f64| x.ln());

#[signature(
    math.log,
    output = Known(ValueType::Float),
    short = "The logarithm of number in base.")]
pub struct Log {
    number: Number,
    base: Number,
}

fn log(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Log = Log::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Float(cfg.number.as_float().log(cfg.base.as_float())))
}

#[signature(
    math.pow,
    output = Known(ValueType::Float),
    short = "Raise the number to n.")]
pub struct Pow {
    base: Number,
    n: Number,
}

fn pow(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Pow = Pow::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Float(cfg.base.as_float().powf(cfg.n.as_float())))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "math",
        "Math commands",
        Box::new(move |env| {
            Sin::declare(env)?;
            Cos::declare(env)?;
            Tan::declare(env)?;
            Sqrt::declare(env)?;
            ASin::declare(env)?;
            ACos::declare(env)?;
            ATan::declare(env)?;
            Ln::declare(env)?;
            Floor::declare(env)?;
            Ceil::declare(env)?;
            Log::declare(env)?;
            Pow::declare(env)?;
            env.declare("pi", Value::Float(std::f64::consts::PI))?;
            env.declare("tau", Value::Float(std::f64::consts::PI * 2.0))?;
            env.declare("e", Value::Float(std::f64::consts::E))?;
            Ok(())
        }),
    )?;
    Ok(())
}
