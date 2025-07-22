use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::signature::number::Number;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
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
    })
}

#[signature(
    types.float.__add__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Add this number and the specified term and return the result",
)]
#[allow(unused)]
struct Add {
    #[description("the number to add.")]
    term: Number,
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
    types.float.__sub__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Subtract the specified term from this number and return the result",
)]
#[allow(unused)]
struct Sub {
    #[description("the number to subtract")]
    term: Number,
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
    types.float.__mul__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "multiply this number and the specified factor and return the result",
)]
#[allow(unused)]
struct Mul {
    #[description("the number to multiply")]
    term: Number,
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
    types.float.__div__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Divide this number by the specified factor",
)]
#[allow(unused)]
struct Div {
    #[description("the number to divide by")]
    term: Number,
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
    types.float.__neg__,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Negate this float",
)]
struct Neg {}

fn __neg__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(-context.this.float(&context.source)?))
}

#[signature(
    types.float.is_nan,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if this float is NaN",
)]
struct IsNan {}

fn is_nan(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.float(&context.source)?.is_nan()))
}

#[signature(
    types.float.is_infinite,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if this float is positive or negative infinity.",
)]
struct IsInfinite {}

fn is_infinite(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Bool(context.this.float(&context.source)?.is_infinite()))
}

#[signature(
    types.float.max,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Largest finite float value",
)]
struct Max {}

fn max(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(f64::MAX))
}

#[signature(
    types.float.min,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Smallest finite float value",
)]
struct Min {}

fn min(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(f64::MIN))
}

#[signature(
    types.float.nan,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Not a number",
)]
struct Nan {}

fn nan(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(f64::NAN))
}

#[signature(
    types.float.infinity,
    can_block = false,
    output = Known(ValueType::Float),
    short = "Infinity",
)]
struct Infinity {}

fn infinity(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Float(f64::INFINITY))
}
