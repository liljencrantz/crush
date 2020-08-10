use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, to_crush_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, This, ValueExecutionContext};
use crate::lang::value::ValueType;
use crate::lang::{execution_context::ExecutionContext, value::Value};
use chrono::{DateTime, Datelike, Local, Timelike};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use std::cmp::max;
use time::strptime;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "time", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "string"];
        res.declare(
            full("__add__"),
            add,
            false,
            "time + delta:duration",
            "Add the specified delta to this time",
            None,
            Known(ValueType::Time),
        );
        res.declare(
            full("__sub__"),
            sub,
            false,
            "time - delta:duration",
            "Remove the specified delta from this time",
            None,
            Known(ValueType::Time),
        );
        res.declare(
            full("now"),
            now,
            false,
            "time:now",
            "The current point in time",
            None,
            Known(ValueType::Time),
        );
        Parse::declare_method(&mut res, &path);
        res
    };
}

binary_op!(add, time, Duration, Time, |a, b| a + b);
binary_op!(
    sub,
    time,
    Duration,
    Time,
    |a, b| a - b,
    Time,
    Duration,
    |a, b| a - b
);

fn now(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Time(Local::now()))
}

#[signature(
parse,
can_block=false,
output=Known(ValueType::Time),
short="Parse a time string using a strptime-style pattern string")]
struct Parse {
    #[description("the format of the time.")]
    format: String,
    #[description("the time string to parse.")]
    time: String,
}

fn parse(context: ExecutionContext) -> CrushResult<()> {
    let cfg: Parse = Parse::parse(context.arguments, &context.printer)?;
    let tm = to_crush_error(strptime(&cfg.time, cfg.format.as_ref()))?;
    let dt = Local::now()
        .with_year(tm.tm_year + 1900)
        .unwrap()
        .with_month0(tm.tm_mon as u32)
        .unwrap()
        .with_day(max(tm.tm_mday as u32, 1))
        .unwrap()
        .with_hour(tm.tm_hour as u32)
        .unwrap()
        .with_minute(tm.tm_min as u32)
        .unwrap()
        .with_second(tm.tm_sec as u32)
        .unwrap()
        .with_nanosecond(tm.tm_nsec as u32)
        .unwrap();
    context.output.send(Value::Time(dt))
}
