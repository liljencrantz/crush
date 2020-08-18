use crate::lang::command::OutputType::Known;
use crate::lang::errors::argument_error;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::ArgumentVector;
use crate::lang::execution_context::CommandContext;
use crate::lang::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;

macro_rules! math_fun {
    ($name:ident, $op:expr) => {
        fn $name(mut context: CommandContext) -> CrushResult<()> {
            context.arguments.check_len(1)?;
            let x = match context.arguments.value(0)? {
                Value::Float(f) => f,
                Value::Integer(i) => i as f64,
                v => {
                    return argument_error(
                        format!("Expected a number, got a {}", v.value_type().to_string()).as_str(),
                    )
                }
            };
            context.output.send(Value::Float($op(x)))
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
                    return argument_error(
                        format!("Expected a number, got a {}", v.value_type().to_string()).as_str(),
                    )
                }
            };
            let y = match context.arguments.value(1)? {
                Value::Float(f) => f,
                Value::Integer(i) => i as f64,
                v => {
                    return argument_error(
                        format!("Expected a number, got a {}", v.value_type().to_string()).as_str(),
                    )
                }
            };
            context.output.send(Value::Float($op(x, y)))
        }
    };
}

math_fun!(sin, |x: f64| x.sin());
math_fun!(cos, |x: f64| x.cos());
math_fun!(tan, |x: f64| x.tan());
math_fun!(sqrt, |x: f64| x.sqrt());
math_fun!(asin, |x: f64| x.asin());
math_fun!(acos, |x: f64| x.acos());
math_fun!(atan, |x: f64| x.atan());
math_fun!(ceil, |x: f64| x.ceil());
math_fun!(floor, |x: f64| x.floor());
math_fun!(ln, |x: f64| x.ln());
math_fun2!(pow, |x: f64, y: f64| x.powf(y));
math_fun2!(log, |x: f64, y: f64| x.log(y));

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_lazy_namespace(
        "math",
        Box::new(move |env| {
            env.declare_command(
                "sin",
                sin,
                false,
                "math:sin angle:float",
                "The sine of the specified angle",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "cos",
                cos,
                false,
                "math:cos angle:float",
                "The cosine of the specified angle",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "tan",
                tan,
                false,
                "math:tan angle:float",
                "The tangent of the specified angle",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "sqrt",
                sqrt,
                false,
                "math:sqrt angle:float",
                "The square root of the specified angle",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "asin",
                asin,
                false,
                "math:asin arc:float",
                "The inverse sine of the specified arc",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "acos",
                acos,
                false,
                "math:acos arc:float",
                "The inverse cosine of the specified arc",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "atan",
                atan,
                false,
                "math:atan arc:float",
                "The inverse tangent of the specified arc",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "pow",
                pow,
                false,
                "math:pow number:float n:float",
                "Raise the number to n",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "log",
                log,
                false,
                "math:log number:float base:float",
                "The logarithm of number in the specified base",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "ln",
                ln,
                false,
                "math:ln number:float",
                "The natural logarithm of number",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "floor",
                floor,
                false,
                "math:floor number:float",
                "The largest integer smaller than number",
                None,
                Known(ValueType::Float),
            )?;
            env.declare_command(
                "ceil",
                ceil,
                false,
                "math:ceil number:float",
                "The smallest integer larger than number",
                None,
                Known(ValueType::Float),
            )?;
            env.declare("pi", Value::Float(std::f64::consts::PI))?;
            env.declare("tau", Value::Float(std::f64::consts::PI * 2.0))?;
            env.declare("e", Value::Float(std::f64::consts::E))?;
            Ok(())
        }),
    )?;
    Ok(())
}
