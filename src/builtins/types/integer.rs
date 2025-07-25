use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
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
        Mod::declare_method(&mut res);
        Rem::declare_method(&mut res);
        Neg::declare_method(&mut res);
        Max::declare_method(&mut res);
        Min::declare_method(&mut res);
        res
    })
}

#[signature(
    types.integer.__add__,
    can_block = false,
    output = Unknown,
    short = "Add this number and the specified term and return the result",
)]
#[allow(unused)]
struct Add {
    #[description("the number to add")]
    term: Number,
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
#[allow(unused)]
struct Sub {
    #[description("the number to subtract")]
    term: Number,
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
#[allow(unused)]
struct Mul {
    #[description("the number to multiply")]
    term: Number,
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
#[allow(unused)]
struct Div {
    #[description("the number to divide by")]
    term: Number,
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
#[allow(unused)]
struct Rem {
    #[description("the number to divide by")]
    term: i128,
}

binary_op!(rem, integer, Integer, Integer, |a, b| a % b);

#[signature(
    types.integer.r#mod,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Least positive residue after integer division",
)]
#[allow(unused)]
struct Mod {
    #[description("the number to divide by")]
    term: i128,
}

binary_op!(r#mod, integer, Integer, Integer, |a, b| (a % b + b) % b);

#[signature(
    types.integer.__neg__,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Negate this integer",
)]
#[allow(unused)]
struct Neg {}

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
#[allow(unused)]
struct Max {}

fn max(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(i128::MAX))
}

#[signature(
    types.integer.min,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Smallest integer value",
)]
#[allow(unused)]
struct Min {}

fn min(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(i128::MIN))
}
