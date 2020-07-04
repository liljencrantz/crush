use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use crate::lang::ordered_map::OrderedMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "integer", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: OrderedMap<String, Box<dyn CrushCommand +  Send + Sync>> = OrderedMap::new();
        res.declare(full("__add__"),
            add, false,
            "integer + term:(integer|float)",
            "Add this number by the specified term",
            None);
        res.declare(full("__sub__"),
            sub, false,
            "integer - term:(integer|float)",
            "Subtract the specified term from this number",
            None);
        res.declare(full("__mul__"),
            mul, false,
            "integer * factor:(integer|float)", "Multiply this number with the specified factor",
            None);
        res.declare(
            full("__div__"), div, false,
            "integer / factor:(integer|float)", "Divide this number by the specified factor",
            None);
        res.declare(
            full("mod"), r#mod, false,
            "integer:mod factor:integer", "Least positive residue after integer division",
            None);
        res.declare(
            full("rem"), rem, false,
            "integer:rem factor:integer", "Remainder after integer division",
            None);
        res.declare(
            full("__neg__"), neg, false,
            "neg integer", "Negate this integer",
            None);
        res
    };
}

binary_op!(add, integer, Integer, Integer, |a, b| a+b, Float, Float, |a, b| a as f64+b);
binary_op!(sub, integer, Integer, Integer, |a, b| a-b, Float, Float, |a, b| a as f64-b);
binary_op!(mul, integer, Integer, Integer, |a, b| a*b, Float, Float, |a, b| a as f64*b);
binary_op!(div, integer, Integer, Integer, |a, b| a/b, Float, Float, |a, b| a as f64/b);
binary_op!(rem, integer, Integer, Integer, |a, b| a % b);
binary_op!(r#mod, integer, Integer, Integer, |a, b| (a % b + b) % b);

fn neg(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(-context.this.integer()?))
}
