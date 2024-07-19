use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
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
use crate::lang::command::OutputType::Unknown;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Add::declare_method(&mut res);
        Sub::declare_method(&mut res);
        Mul::declare_method(&mut res);
        Div::declare_method(&mut res);
        Mod::declare_method(&mut res);
        Rem::declare_method(&mut res);
        Neg::declare_method(&mut res);
        Max::declare_method(&mut res);
        Min::declare_method(&mut res);

        res
    };
}

#[signature(
    types.integer.__add__,
    can_block = false,
    output = Unknown,
    short = "Add this number and the specified term and return the result",
)]
struct Add {
    #[description("the number to add")]
    term: Number
}

binary_op!(
    __add__,
    integer,
    Integer,
    Integer,
    |a, b| a + b,
    Float,
    Float,
    |a, b| a as f64 + b
);

#[signature(
    types.integer.__sub__,
    can_block = false,
    output = Unknown,
    short = "Subtract the specified term from this number and return the result",
)]
struct Sub {
    #[description("the number to subtract")]
    term: Number
}
binary_op!(
    __sub__,
    integer,
    Integer,
    Integer,
    |a, b| a - b,
    Float,
    Float,
    |a, b| a as f64 - b
);

#[signature(
    types.integer.__mul__,
    can_block = false,
    output = Unknown,
    short = "multiply this number and the specified factor and return the result",
)]
struct Mul {
    #[description("the number to multiply")]
    term: Number
}

binary_op!(
    __mul__,
    integer,
    Integer,
    Integer,
    |a, b| a * b,
    Float,
    Float,
    |a, b| a as f64 * b
);

#[signature(
    types.integer.__div__,
    can_block = false,
    output = Unknown,
    short = "Divide this number by the specified factor",
)]
struct Div {
    #[description("the number to divide by")]
    term: Number
}
binary_op!(
    __div__,
    integer,
    Integer,
    Integer,
    |a, b| a / b,
    Float,
    Float,
    |a, b| a as f64 / b
);

#[signature(
    types.integer.rem,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Remainder after integer division",
)]
struct Rem {
    #[description("the number to divide by")]
    term: i128
}

binary_op!(rem, integer, Integer, Integer, |a, b| a % b);

#[signature(
    types.integer.r#mod,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Least positive residue after integer division",
)]
struct Mod {
    #[description("the number to divide by")]
    term: i128
}

binary_op!(r#mod, integer, Integer, Integer, |a, b| (a % b + b) % b);

#[signature(
    types.integer.__neg__,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Negate this integer",
)]
struct Neg {
}

fn __neg__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(-context.this.integer()?))
}

#[signature(
    types.integer.max,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Largest integer value",
)]
struct Max {
}

fn max(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(i128::MAX))
}

#[signature(
    types.integer.min,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Smallest integer value",
)]
struct Min {
}

fn min(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(i128::MIN))
}
