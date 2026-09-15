use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::number::Number;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use signature::signature;

fn number_to_value(n: Number) -> Value {
    match n {
        Number::Integer(i) => Value::Integer(i),
        Number::Float(f) => Value::Float(f),
    }
}

macro_rules! math_fun {
    ($name:ident, $Signature: ident, $op:expr) => {
        fn $name(mut context: CommandContext) -> CrushResult<()> {
            let cfg: $Signature =
                $Signature::parse(context.remove_arguments(), &context.global_state.printer())?;
            context
                .output
                .send(Value::Float($op(cfg.number.as_float())))
        }
    };
}

#[signature(
    math.sin,
    output = Known(ValueType::Float),
    short = "The sine of number.",
    example = "math:sin 1",
)]
pub struct Sin {
    #[description("the number to take the sine of, in radians.")]
    number: Number,
}
math_fun!(sin, Sin, |x: f64| x.sin());

#[signature(
    math.cos,
    output = Known(ValueType::Float),
    short = "The cosine of number.",
    example = "math:cos 1",
)]
pub struct Cos {
    #[description("the number to take the cosine of, in radians.")]
    number: Number,
}
math_fun!(cos, Cos, |x: f64| x.cos());

#[signature(
    math.tan,
    output = Known(ValueType::Float),
    short = "The tangent of number.")]
pub struct Tan {
    #[description("the number to take the tangent of, in radians.")]
    number: Number,
}
math_fun!(tan, Tan, |x: f64| x.tan());

#[signature(
    math.sqrt,
    output = Known(ValueType::Float),
    short = "The square root of number.")]
pub struct Sqrt {
    #[description("the number to take the square root of.")]
    number: Number,
}
math_fun!(sqrt, Sqrt, |x: f64| x.sqrt());

#[signature(
    math.asin,
    output = Known(ValueType::Float),
    short = "The arc sine of number.")]
pub struct ASin {
    #[description("the value, between -1 and 1, to take the arc sine of.")]
    number: Number,
}
math_fun!(asin, ASin, |x: f64| x.asin());

#[signature(
    math.acos,
    output = Known(ValueType::Float),
    short = "The arc cosine of number.")]
pub struct ACos {
    #[description("the value, between -1 and 1, to take the arc cosine of.")]
    number: Number,
}
math_fun!(acos, ACos, |x: f64| x.acos());

#[signature(
    math.atan,
    output = Known(ValueType::Float),
    short = "The arc tangent of number.")]
pub struct ATan {
    #[description("the value to take the arc tangent of.")]
    number: Number,
}
math_fun!(atan, ATan, |x: f64| x.atan());

#[signature(
    math.ceil,
    output = Known(ValueType::Float),
    short = "The smallest integer larger than number.")]
pub struct Ceil {
    #[description("the number to round up to the nearest integer.")]
    number: Number,
}
math_fun!(ceil, Ceil, |x: f64| x.ceil());

#[signature(
    math.floor,
    output = Known(ValueType::Float),
    short = "The largest integer smaller than number.")]
pub struct Floor {
    #[description("the number to round down to the nearest integer.")]
    number: Number,
}
math_fun!(floor, Floor, |x: f64| x.floor());

#[signature(
    math.ln,
    output = Known(ValueType::Float),
    short = "The natural logarithm of number.")]
pub struct Ln {
    #[description("the number to take the natural logarithm of.")]
    number: Number,
}
math_fun!(ln, Ln, |x: f64| x.ln());

#[signature(
    math.exp,
    output = Known(ValueType::Float),
    short = "e (Euler's number) raised to the power of number.",
    long = "The same result is available as `math:pow math:e number`, but `exp` doesn't \
    require knowing about the `e` constant, and its dedicated implementation is generally \
    more numerically precise than going through a general-purpose `pow`.",
    example = "# Returns 1, since e^0 is 1",
    example = "math:exp 0",
)]
pub struct Exp {
    #[description("the exponent to raise e to.")]
    number: Number,
}
math_fun!(exp, Exp, |x: f64| x.exp());

#[signature(
    math.log,
    output = Known(ValueType::Float),
    short = "The logarithm of number in base.")]
pub struct Log {
    #[description("the number to take the logarithm of.")]
    number: Number,
    #[description("the base of the logarithm.")]
    base: Number,
}

fn log(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Log = Log::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::Float(cfg.number.as_float().log(cfg.base.as_float())))
}

#[signature(
    math.pow,
    output = Known(ValueType::Float),
    short = "Raise the number to n.")]
pub struct Pow {
    #[description("the base to raise to a power.")]
    base: Number,
    #[description("the exponent to raise the base to.")]
    n: Number,
}

fn pow(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Pow = Pow::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::Float(cfg.base.as_float().powf(cfg.n.as_float())))
}

#[signature(
    math.abs,
    output = Unknown,
    short = "The absolute value of number.",
    example = "math:abs -5",
)]
pub struct Abs {
    #[description("the number to take the absolute value of.")]
    number: Number,
}

fn abs(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Abs = Abs::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(match cfg.number {
        // i128::MIN has no positive counterpart representable as an i128 -- i.abs()
        // panics on it unless checked explicitly.
        Number::Integer(i) => match i.checked_abs() {
            Some(res) => Value::Integer(res),
            None => return command_error("Integer overflow"),
        },
        Number::Float(f) => Value::Float(f.abs()),
    })
}

#[signature(
    math.sign,
    output = Unknown,
    short = "-1, 0 or 1 depending on the sign of number.",
    example = "math:sign -5",
)]
pub struct Sign {
    #[description("the number to inspect the sign of.")]
    number: Number,
}

fn sign(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Sign = Sign::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(match cfg.number {
        Number::Integer(i) => Value::Integer(i.signum()),
        Number::Float(f) => Value::Float(if f == 0.0 { 0.0 } else { f.signum() }),
    })
}

#[signature(
    math.round,
    output = Known(ValueType::Float),
    short = "Number rounded to the nearest whole number.")]
pub struct Round {
    #[description("the number to round to the nearest whole number.")]
    number: Number,
}
math_fun!(round, Round, |x: f64| x.round());

#[signature(
    math.clamp,
    output = Unknown,
    short = "Number restricted to the inclusive range [min, max].",
    example = "math:clamp 15 min=0 max=10",
)]
pub struct Clamp {
    #[description("the number to clamp.")]
    number: Number,
    #[description("the lower bound of the allowed range.")]
    min: Number,
    #[description("the upper bound of the allowed range.")]
    max: Number,
}

fn clamp(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Clamp = Clamp::parse(context.remove_arguments(), &context.global_state.printer())?;
    if cfg.number.as_float() < cfg.min.as_float() {
        context.output.send(number_to_value(cfg.min))
    } else if cfg.number.as_float() > cfg.max.as_float() {
        context.output.send(number_to_value(cfg.max))
    } else {
        context.output.send(number_to_value(cfg.number))
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "math",
        "Math commands",
        None,
        Box::new(move |env| {
            Sin::declare(env)?;
            Cos::declare(env)?;
            Tan::declare(env)?;
            Sqrt::declare(env)?;
            ASin::declare(env)?;
            ACos::declare(env)?;
            ATan::declare(env)?;
            Ln::declare(env)?;
            Exp::declare(env)?;
            Floor::declare(env)?;
            Ceil::declare(env)?;
            Log::declare(env)?;
            Pow::declare(env)?;
            Abs::declare(env)?;
            Sign::declare(env)?;
            Round::declare(env)?;
            Clamp::declare(env)?;
            env.declare("pi", Value::Float(std::f64::consts::PI))?;
            env.declare("tau", Value::Float(std::f64::consts::PI * 2.0))?;
            env.declare("e", Value::Float(std::f64::consts::E))?;
            Ok(())
        }),
    )?;
    Ok(())
}
