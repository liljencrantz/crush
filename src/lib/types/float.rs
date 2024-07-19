use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::signature::number::Number;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::this::This;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Add::declare_method(&mut res);
        Sub::declare_method(&mut res);
        Mul::declare_method(&mut res);
        Div::declare_method(&mut res);
        Neg::declare_method(&mut res);
        IsInfinite::declare_method(&mut res);
        IsNan::declare_method(&mut res);
        Max::declare_method(&mut res);
        Min::declare_method(&mut res);
        Nan::declare_method(&mut res);
        Infinity::declare_method(&mut res);

        res
    };
}

#[signature(
    __add__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Add this number and the specified term and return the result",
    path = ("types", "float"),
)]
struct Add {
    #[description("the number to add")]
    term: Number
}

binary_op!(
    __add__,
    float,
    Integer,
    Float,
    |a, b| a + (b as f64),
    Float,
    Float,
    |a, b| a + b
);

#[signature(
    __sub__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Subtract the specified term from this number and return the result",
    path = ("types", "float"),
)]
struct Sub {
    #[description("the number to subtract")]
    term: Number
}

binary_op!(
    __sub__,
    float,
    Integer,
    Float,
    |a, b| a - (b as f64),
    Float,
    Float,
    |a, b| a - b
);

#[signature(
    __mul__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "multiply this number and the specified factor and return the result",
    path = ("types", "float"),
)]
struct Mul {
    #[description("the number to multiply")]
    term: Number
}

binary_op!(
    __mul__,
    float,
    Integer,
    Float,
    |a, b| a * (b as f64),
    Float,
    Float,
    |a, b| a * b
);

#[signature(
    __div__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Divide this number by the specified factor",
    path = ("types", "float"),
)]
struct Div {
    #[description("the number to divide by")]
    term: Number
}
binary_op!(
    __div__,
    float,
    Integer,
    Float,
    |a, b| a / (b as f64),
    Float,
    Float,
    |a, b| a / b
);

#[signature(
    __neg__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Negate this float",
    path = ("types", "float"),
)]
struct Neg {
}

fn __neg__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(-context.this.float()?))
}

#[signature(
    is_nan,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if this float is NaN",
    path = ("types", "float"),
)]
struct IsNan {
}

fn is_nan(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.float()?.is_nan()))
}

#[signature(
    is_infinite,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if this float is positive or negative infinity.",
    path = ("types", "float"),
)]
struct IsInfinite {}

fn is_infinite(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.float()?.is_infinite()))
}

#[signature(
    max,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Largest finite float value",
    path = ("types", "float"),
)]
struct Max {
}

fn max(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Float(f64::MAX))
}

#[signature(
    min,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Smallest finite float value",
    path = ("types", "float"),
)]
struct Min {
}

fn min(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Float(f64::MIN))
}

#[signature(
    nan,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Not a number",
    path = ("types", "float"),
)]
struct Nan {
}

fn nan(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Float(f64::NAN))
}

#[signature(
    infinity,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Infinity",
    path = ("types", "float"),
)]
struct Infinity {
}

fn infinity(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Float(f64::INFINITY))
}
