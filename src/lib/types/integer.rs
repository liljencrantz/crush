use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::execution_context::{ArgumentVector, This};
use crate::lang::value::ValueType;
use crate::lang::{execution_context::CommandContext, value::Value};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "integer", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        res.declare(
            full("__add__"),
            add,
            false,
            "integer + term:(integer|float)",
            "Add this number by the specified term",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("__sub__"),
            sub,
            false,
            "integer - term:(integer|float)",
            "Subtract the specified term from this number",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("__mul__"),
            mul,
            false,
            "integer * factor:(integer|float)",
            "Multiply this number with the specified factor",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("__div__"),
            div,
            false,
            "integer / factor:(integer|float)",
            "Divide this number by the specified factor",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("mod"),
            r#mod,
            false,
            "integer:mod factor:integer",
            "Least positive residue after integer division",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("rem"),
            rem,
            false,
            "integer:rem factor:integer",
            "Remainder after integer division",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("__neg__"),
            neg,
            false,
            "neg integer",
            "Negate this integer",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("max"),
            max,
            false,
            "integer:max",
            "Largest integer value",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res.declare(
            full("min"),
            min,
            false,
            "float:min",
            "Smallest integer value",
            None,
            Known(ValueType::Integer),
            vec![],
        );
        res
    };
}

binary_op!(
    add,
    integer,
    Integer,
    Integer,
    |a, b| a + b,
    Float,
    Float,
    |a, b| a as f64 + b
);
binary_op!(
    sub,
    integer,
    Integer,
    Integer,
    |a, b| a - b,
    Float,
    Float,
    |a, b| a as f64 - b
);
binary_op!(
    mul,
    integer,
    Integer,
    Integer,
    |a, b| a * b,
    Float,
    Float,
    |a, b| a as f64 * b
);
binary_op!(
    div,
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
