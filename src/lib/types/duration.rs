use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, command::ExecutionContext};
use crate::lang::command::{CrushCommand, ArgumentVector, This};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::value::ValueType;
use crate::lib::binary_op;
use chrono::Duration;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("__add__"), CrushCommand::command(add, false));
        res.insert(Box::from("__sub__"), CrushCommand::command(sub, false));
        res.insert(Box::from("__mul__"), CrushCommand::command(mul, false));
        res.insert(Box::from("__div__"), CrushCommand::command(div, false));
        res.insert(Box::from("new"), CrushCommand::command(new, false));
        res.insert(Box::from("__neg__"), CrushCommand::command(neg, false));
        res
    };
}

binary_op!(add, duration, Duration, Duration, |a, b| a+b, Time, Time, |a, b| b+a);
binary_op!(sub, duration, Duration, Duration, |a, b| a-b);
binary_op!(mul, duration, Integer, Duration, |a, b| a*(b as i32));
binary_op!(div, duration, Integer, Duration, |a, b| a/(b as i32));

fn to_duration(a: i64, t: &str) -> CrushResult<chrono::Duration> {
    match t {
        "nanosecond" | "nanoseconds" => Ok(Duration::nanoseconds(a)),
        "microsecond" | "microseconds" => Ok(Duration::microseconds(a)),
        "millisecond" | "milliseconds" => Ok(Duration::milliseconds(a)),
        "second" | "seconds" => Ok(Duration::seconds(a)),
        "minute" | "minutes" => Ok(Duration::seconds(a * 60)),
        "hour" | "hours" => Ok(Duration::seconds(a * 3600)),
        "day" | "days" => Ok(Duration::seconds(a * 3600 * 24)),
        "year" | "years" => Ok(Duration::seconds(a * 3600 * 24 * 365)),
        _ => argument_error("Invalid duration"),
    }
}

fn new(mut context: ExecutionContext) -> CrushResult<()> {
    let v: Vec<Value> = context.arguments.drain(..)
        .map(|a| a.value)
        .collect::<Vec<Value>>();
    if v.len() % 2 == 0 {
        let vec = v.chunks(2)
            .map(|chunks| match (&chunks[0], &chunks[1]) {
                (Value::Integer(a), Value::String(t)) => to_duration(*a as i64, t.as_ref()),
                _ => argument_error("Unknown duration format"),
            })
            .collect::<CrushResult<Vec<Duration>>>()?;
        let mut res = Duration::seconds(0);
        vec.iter()
            .for_each(|d| {
                res = res + d.clone();
            });
        context.output.send(Value::Duration(res))
    } else {
        argument_error("Unknown duration format")
    }
}

fn neg(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Duration(-context.this.duration()?))
}
