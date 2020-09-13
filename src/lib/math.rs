use crate::lang::command::OutputType::Known;
use crate::lang::errors::argument_error_legacy;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::ArgumentVector;
use crate::lang::execution_context::CommandContext;
use crate::lang::data::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use signature::signature;
use crate::lang::number::Number;

macro_rules! math_fun {
    ($name:ident, $Signature: ident, $op:expr) => {
        fn $name(context: CommandContext) -> CrushResult<()> {
            let cfg: $Signature = $Signature::parse(context.arguments, &context.printer)?;
            context.output.send(Value::Float($op(cfg.number.as_float())))
        }
    };
}

macro_rules! math_fun2 {
    ($name:ident, $op:expr) => {
        fn $name(mut context: CommandContext) -> CrushResult<()> {
            context.arguments.check_len(2)?;
            let x = match context.arguments.value(0)? {
                Value::Float(f) => f,
                Value::Integer(i) => i as f64,
                v => {
                    return argument_error_legacy(
                        &format!("Expected a number, got a {}", v.value_type()),
                    )
                }
            };
            let y = match context.arguments.value(1)? {
                Value::Float(f) => f,
                Value::Integer(i) => i as f64,
                v => {
                    return argument_error_legacy(
                        &format!("Expected a number, got a {}", v.value_type()),
                    )
                }
            };
            context.output.send(Value::Float($op(x, y)))
        }
    };
}

#[signature(
sin,
output = Known(ValueType::Float),
short = "The sine of number.")]
pub struct Sin {
    number: Number,
}
math_fun!(sin, Sin, |x: f64| x.sin());

#[signature(
cos,
output = Known(ValueType::Float),
short = "The cosine of number.")]
pub struct Cos {
    number: Number,
}
math_fun!(cos, Cos, |x: f64| x.cos());

#[signature(
tan,
output = Known(ValueType::Float),
short = "The tangent of number.")]
pub struct Tan {
    number: Number,
}
math_fun!(tan, Tan, |x: f64| x.tan());

#[signature(
sqrt,
output = Known(ValueType::Float),
short = "The square root of number.")]
pub struct Sqrt {
    number: Number,
}
math_fun!(sqrt, Sqrt, |x: f64| x.sqrt());

#[signature(
asin,
output = Known(ValueType::Float),
short = "The arc sine of number.")]
pub struct ASin {
    number: Number,
}
math_fun!(asin, ASin, |x: f64| x.asin());

#[signature(
acos,
output = Known(ValueType::Float),
short = "The arc cosineof  number.")]
pub struct ACos {
    number: Number,
}
math_fun!(acos, ACos, |x: f64| x.acos());

#[signature(
atan,
output = Known(ValueType::Float),
short = "The arc tangent of number.")]
pub struct ATan {
    number: Number,
}
math_fun!(atan, ATan, |x: f64| x.atan());

#[signature(
ceil,
output = Known(ValueType::Float),
short = "The smallest integer larger than number.")]
pub struct Ceil {
    number: Number,
}
math_fun!(ceil, Ceil, |x: f64| x.ceil());

#[signature(
floor,
output = Known(ValueType::Float),
short = "The largest integer smaller than number.")]
pub struct Floor {
    number: Number,
}
math_fun!(floor, Floor, |x: f64| x.floor());

#[signature(
ln,
output = Known(ValueType::Float),
short = "The natural logarithm of number.")]
pub struct Ln {
    number: Number,
}
math_fun!(ln, Ln, |x: f64| x.ln());

math_fun2!(pow, |x: f64, y: f64| x.powf(y));
math_fun2!(log, |x: f64, y: f64| x.log(y));

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "math",
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
            env.declare_command(
                "pow",
                pow,
                false,
                "math:pow number:float n:float",
                "Raise the number to n",
                None,
                Known(ValueType::Float),
                vec![],
            )?;
            env.declare_command(
                "log",
                log,
                false,
                "math:log number:float base:float",
                "The logarithm of number in the specified base",
                None,
                Known(ValueType::Float),
                vec![],
            )?;
            env.declare("pi", Value::Float(std::f64::consts::PI))?;
            env.declare("tau", Value::Float(std::f64::consts::PI * 2.0))?;
            env.declare("e", Value::Float(std::f64::consts::E))?;
            Ok(())
        }),
    )?;
    Ok(())
}
