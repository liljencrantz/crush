use crate::lang::errors::{CrushResult, argument_error, to_crush_error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use crate::lang::ordered_map::OrderedMap;
use lazy_static::lazy_static;
use chrono::{Local, Datelike, Timelike};
use time::strptime;
use std::cmp::max;
use crate::lang::command::CrushCommand;
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "time", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: OrderedMap<String, Box<dyn CrushCommand +  Send + Sync>> = OrderedMap::new();
        res.declare(full("__add__"),
            add, false,
            "time + delta:duration",
            "Add the specified delta to this time",
            None);
        res.declare(
            full("__sub__"), sub, false,
            "time - delta:duration", "Remove the specified delta from this time", None);
        res.declare(
            full("now"), now, false,"time:now", "The current point in time", None);
        res.declare(full("parse"),
            parse, false,
            "time:parse format=format:string time=time:string",
            "Parse a time string using a strptime-style pattern string",
            None
            );
        res
    };
}

binary_op!(add, time, Duration, Time, |a, b| a+b);
binary_op!(sub, time, Duration, Time, |a, b| a-b, Time, Duration, |a, b| a-b);

fn now(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Time(Local::now()))
}

fn parse(mut context: ExecutionContext) -> CrushResult<()> {
    let mut tm: Option<String> = None;
    let mut fmt: Option<String> = None;

    for arg in context.arguments.drain(..) {
        match (arg.argument_type.as_deref().unwrap_or(""), arg.value) {
            ("format", Value::String(s)) => fmt = Some(s),
            ("time", Value::String(s)) => tm = Some(s),
            _ => return argument_error("Invalid argument"),
        }
    }

    match (tm, fmt) {
        (Some(t), Some(f)) => {
            let tm = to_crush_error(strptime(t.as_ref(), f.as_ref()))?;
            let dt = Local::now()
                .with_year(tm.tm_year + 1900).unwrap()
                .with_month0(tm.tm_mon as u32).unwrap()
                .with_day(max(tm.tm_mday as u32, 1)).unwrap()
                .with_hour(tm.tm_hour as u32).unwrap()
                .with_minute(tm.tm_min as u32).unwrap()
                .with_second(tm.tm_sec as u32).unwrap()
                .with_nanosecond(tm.tm_nsec as u32).unwrap();
            context.output.send(Value::Time(dt))
        }
        _ => argument_error("Must specify both time and format"),
    }
}
