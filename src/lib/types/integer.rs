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
use crate::lang::command::OutputType::Unknown;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "integer", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Add::declare_method(&mut res);
        Sub::declare_method(&mut res);
        Mul::declare_method(&mut res);
        Div::declare_method(&mut res);
        res.declare(
            full("mod"),
            r#mod,
            false,
            "integer:mod factor:integer",
            "Least positive residue after integer division",
            None,
            Known(ValueType::Integer),
            [],
        );
        res.declare(
            full("rem"),
            rem,
            false,
            "integer:rem factor:integer",
            "Remainder after integer division",
            None,
            Known(ValueType::Integer),
            [],
        );
        res.declare(
            full("__neg__"),
            neg,
            false,
            "neg integer",
            "Negate this integer",
            None,
            Known(ValueType::Integer),
            [],
        );
        res.declare(
            full("max"),
            max,
            false,
            "integer:max",
            "Largest integer value",
            None,
            Known(ValueType::Integer),
            [],
        );
        res.declare(
            full("min"),
            min,
            false,
            "float:min",
            "Smallest integer value",
            None,
            Known(ValueType::Integer),
            [],
        );
        res
    };
}

#[signature(
    __add__,
    can_block = false,
    output = Unknown,
    short = "Add this number and the specified term and return the result",
    path = ("types", "integer"),
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
    __sub__,
    can_block = false,
    output = Unknown,
    short = "Subtract the specified term from this number and return the result",
    path = ("types", "integer"),
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
    __mul__,
    can_block = false,
    output = Unknown,
    short = "multiply this number and the specified factor and return the result",
    path = ("types", "integer"),
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
    __div__,
    can_block = false,
    output = Unknown,
    short = "Divide this number by the specified factor",
    path = ("types", "integer"),
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
binary_op!(rem, integer, Integer, Integer, |a, b| a % b);
binary_op!(r#mod, integer, Integer, Integer, |a, b| (a % b + b) % b);

fn neg(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(-context.this.integer()?))
}

fn max(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(i128::MAX))
}

fn min(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(i128::MIN))
}
