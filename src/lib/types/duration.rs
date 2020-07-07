use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use ordered_map::OrderedMap;
use lazy_static::lazy_static;
use chrono::Duration;
use crate::lang::command::Command;
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "duration", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        res.declare(full("__add__"),
            add, false,
            "duration + (delta:duration | time:time)",
            "Add the specified delta or time to this duration",
            None);
        res.declare(full("__sub__"),
            sub, false,
            "duration - delta:duration",
            "Remove the specified delta from this duration",
            None);
        res.declare(full("__mul__"),
            mul, false,
            "duration * factor:integer",
            "Multiply this duration by the specified factor",
            None);
        res.declare(full("__div__"),
            div, false,
            "duration / divisor:integer",
            "Divide this duration by the specified divisor",
            None);
        res.declare(full("new"),
            new, false,
            "duration:new [count:integer timeunit:string]...",
            "Create a new duration",
            Some(r#"    * timeunit:string is one of nanosecond/nanoseconds, microsecond/microseconds,
      millisecond/milliseconds, second/seconds, minute/minutes, hour/hours,
      day/days, week/weeks, month/months, year/years

    Example:

    # A complicated way of specifying a 23 hour duration
    duration:new 1 "days" -3600 "seconds""#));
        res.declare(
            full("__neg__"), neg, false,
            "neg duration", "Negate this duration", None);
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
                res = res + *d;
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
