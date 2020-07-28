use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{value::Value, execution_context::ExecutionContext};
use crate::lang::execution_context::{ArgumentVector, This};
use ordered_map::OrderedMap;
use lazy_static::lazy_static;
use chrono::Duration;
use crate::lang::command::Command;
use crate::lang::command::TypeMap;
use crate::lang::command::OutputType::{Unknown, Known};
use crate::lang::value::ValueType;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "duration", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "duration"];
        res.declare(full("__add__"),
            add, false,
            "duration + (delta:duration | time:time)",
            "Add the specified delta or time to this duration",
            None,
            Unknown);
        res.declare(full("__sub__"),
            sub, false,
            "duration - delta:duration",
            "Remove the specified delta from this duration",
            None,
            Known(ValueType::Duration));
        res.declare(full("__mul__"),
            mul, false,
            "duration * factor:integer",
            "Multiply this duration by the specified factor",
            None,
            Known(ValueType::Duration));
        res.declare(full("__div__"),
            div, false,
            "duration / divisor:integer",
            "Divide this duration by the specified divisor",
            None,
            Known(ValueType::Duration));
        New::declare_method(&mut res, &path);
/*
        res.declare(full("new"),
            new, false,
            "duration:new [count:integer timeunit:string]...",
            "Create a new duration",
            Some(r#"    * timeunit:string is one of nanosecond/nanoseconds, microsecond/microseconds,
      millisecond/milliseconds, second/seconds, minute/minutes, hour/hours,
      day/days, week/weeks, month/months, year/years

    Example:

    # A complicated way of specifying a 23 hour duration
    duration:new 1 "days" (neg 3600) "seconds""#),
    Known(ValueType::Duration));*/
        res.declare(
            full("__neg__"), neg, false,
            "neg duration", "Negate this duration", None,
            Known(ValueType::Duration));
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

#[signature(new, can_block=false, short="Create a new duration")]
struct New {
    #[description("the number of nanoseconds in the duration.")]
    #[default(0i64)]
    nanoseconds: i64,
    #[description("the number of microseconds in the duration.")]
    #[default(0i64)]
    microseconds: i64,
    #[description("the number of milliseconds in the duration.")]
    #[default(0i64)]
    milliseconds: i64,
    #[description("the number of seconds in the duration.")]
    #[default(0i64)]
    seconds: i64,
    #[description("the number of minutes in the duration.")]
    #[default(0i64)]
    minutes: i64,
    #[description("the number of hours in the duration.")]
    #[default(0i64)]
    hours: i64,
    #[description("the number of days in the duration.")]
    #[default(0i64)]
    days: i64,
}

fn new(context: ExecutionContext) -> CrushResult<()> {
    let cfg: New = New::parse(context.arguments, &context.printer)?;

    let res = Duration::nanoseconds(cfg.nanoseconds) +
        Duration::microseconds(cfg.microseconds) +
        Duration::milliseconds(cfg.milliseconds) +
        Duration::seconds(cfg.seconds) +
        Duration::minutes(cfg.minutes) +
        Duration::hours(cfg.hours) +
        Duration::days(cfg.days);
    context.output.send(Value::Duration(res))
}

fn neg(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Duration(-context.this.duration()?))
}
