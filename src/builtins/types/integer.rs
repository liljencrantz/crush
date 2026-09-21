use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::{CrushResult, command_error};
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

// Not implemented via binary_op! like __div__/rem/mod below: integer overflow panics in
// Rust unless checked explicitly, and binary_op!'s $operation is a plain closure with no
// room to return an error instead of a value. Checked here so overflow becomes a normal,
// catchable error instead of a panic. Float addition can't overflow this way -- IEEE 754
// saturates to infinity -- so that arm is unchanged.
fn __add__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.integer()?;
    match context.arguments.value(0)? {
        Value::Integer(v) => match this.checked_add(v) {
            Some(res) => context.output.send(Value::Integer(res)),
            None => command_error("Integer overflow"),
        },
        Value::Float(v) => context.output.send(Value::Float(this as f64 + v)),
        other => command_error(format!(
            "Incompatible argument type `{}` for arithmetic operation.",
            other.value_type()
        )),
    }
}

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

// See __add__ above for why this isn't binary_op!.
fn __sub__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.integer()?;
    match context.arguments.value(0)? {
        Value::Integer(v) => match this.checked_sub(v) {
            Some(res) => context.output.send(Value::Integer(res)),
            None => command_error("Integer overflow"),
        },
        Value::Float(v) => context.output.send(Value::Float(this as f64 - v)),
        other => command_error(format!(
            "Incompatible argument type `{}` for arithmetic operation.",
            other.value_type()
        )),
    }
}

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

// See __add__ above for why this isn't binary_op!.
fn __mul__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.integer()?;
    match context.arguments.value(0)? {
        Value::Integer(v) => match this.checked_mul(v) {
            Some(res) => context.output.send(Value::Integer(res)),
            None => command_error("Integer overflow"),
        },
        Value::Float(v) => context.output.send(Value::Float(this as f64 * v)),
        other => command_error(format!(
            "Incompatible argument type `{}` for arithmetic operation.",
            other.value_type()
        )),
    }
}

#[signature(
    types.integer.__div__,
    can_block = false,
    output = Unknown,
    short = "Divide this number by the specified factor",
    long = "Dividing an integer by an integer zero is an error. Dividing by a float zero",
    long = "follows IEEE 754 (producing infinity or NaN), since the result is a float.",
)]
#[allow(unused)]
struct Div {
    #[description("the number to divide by")]
    term: Number,
}
// Not implemented via binary_op! like the other arithmetic operators: integer division
// by zero panics in Rust (unlike float division, which produces inf/-inf/NaN), and
// binary_op!'s $operation is a plain closure with no room to return an error instead of
// a value. Checked here so it becomes a normal, catchable error instead of a panic.
fn __div__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.integer()?;
    match context.arguments.value(0)? {
        Value::Integer(v) => {
            if v == 0 {
                return command_error("Division by zero");
            }
            context.output.send(Value::Integer(this / v))
        }
        Value::Float(v) => context.output.send(Value::Float(this as f64 / v)),
        other => command_error(format!(
            "Incompatible argument type `{}` for arithmetic operation.",
            other.value_type()
        )),
    }
}

#[signature(
    types.integer.__rem__,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Remainder after integer division",
    long = "A divisor of zero is an error.",
)]
#[allow(unused)]
struct Rem {
    #[description("the number to divide by")]
    term: i128,
}

// Not implemented via binary_op! for the same reason as __div__ above: `a % 0` panics.
fn __rem__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.integer()?;
    match context.arguments.value(0)? {
        Value::Integer(v) => {
            if v == 0 {
                return command_error("Division by zero");
            }
            context.output.send(Value::Integer(this % v))
        }
        other => command_error(format!(
            "Incompatible argument type `{}` for arithmetic operation.",
            other.value_type()
        )),
    }
}

#[signature(
    types.integer.__mod__,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Least positive residue after integer division",
    long = "A divisor of zero is an error.",
)]
#[allow(unused)]
struct Mod {
    #[description("the number to divide by")]
    term: i128,
}

// Not implemented via binary_op! for the same reason as __div__ above: `a % 0` (which
// this is built on) panics.
fn __mod__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.integer()?;
    match context.arguments.value(0)? {
        Value::Integer(v) => {
            if v == 0 {
                return command_error("Division by zero");
            }
            context.output.send(Value::Integer((this % v + v) % v))
        }
        other => command_error(format!(
            "Incompatible argument type `{}` for arithmetic operation.",
            other.value_type()
        )),
    }
}

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
    // i128::MIN has no positive counterpart representable as an i128 -- negating it
    // panics unless checked explicitly, same as the overflow cases in __add__ etc above.
    match context.this.integer()?.checked_neg() {
        Some(res) => context.output.send(Value::Integer(res)),
        None => command_error("Integer overflow"),
    }
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
