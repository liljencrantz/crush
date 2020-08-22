use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, to_crush_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, This};
use crate::lang::value::ValueType;
use crate::lang::{execution_context::CommandContext, value::Value};
use chrono::{Datelike, Local, Timelike};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use std::cmp::max;
use time::{strptime, strftime};

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
        Now::declare_method(&mut res, &path);
        Parse::declare_method(&mut res, &path);
        Format::declare_method(&mut res, &path);
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

#[signature(
now,
can_block = false,
output = Known(ValueType::Time),
short = "The current point in time.",
)]
struct Now {}

fn now(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::Time(Local::now()))
}

#[signature(
parse,
can_block=false,
output=Known(ValueType::Time),
short="Parse a time string using a strptime-style pattern string",
long="The following format codes are recognised in the format string:",
long=" * %a, weekday as abbreviated name.",
long=" * %A, weekday as full name.",
long=" * %w, weekday as a number, where 0 is Sunday and 6 is Saturday.",
long=" * %d, day of month, zero padded.",
long=" * %b, abbreviated month name.",
long=" * %B, month name.",
long=" * %m, month, zero padded.",
long=" * %y, year without century, zero padded.",
long=" * %Y, year with century.",
long=" * %H, hour (24-hour clock) as a zero-padded number.",
long=" * %I, hour (12-hour clock) as a zero-padded number.",
long=" * %p, locale’s equivalent of either AM or PM.",
long=" * %M, minute as a zero-padded number.",
long=" * %S, second as a zero-padded number.",
long=" * %f, microsecond as a decimal number, zero-padded.",
long=" * %z, UTC offset in the form +HHMM or -HHMM.",
long=" * %Z, time zone name.",
long=" * %j, day of the year, zero-padded.",
long=" * %U, week number of the year (Sunday as first day of week).",
long=" * %W, week number of the year (Monday as first day of week).",
long=" * %c, default data and time representation of current locale.",
long=" * %x, default date representation of current locale.",
long=" * %X, default time representation of current locale.",
long=" * %%, a literal % character."
)]
struct Parse {
    #[description("the format of the time.")]
    format: String,
    #[description("the time string to parse.")]
    time: String,
}

fn parse(context: CommandContext) -> CrushResult<()> {
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

#[signature(
format,
can_block=false,
output=Known(ValueType::String),
short="Format this time using a strftime-style pattern string",
long="The following format codes are recognised in the format string:",
long=" * %a, weekday as abbreviated name.",
long=" * %A, weekday as full name.",
long=" * %w, weekday as a number, where 0 is Sunday and 6 is Saturday.",
long=" * %d, day of month, zero padded.",
long=" * %b, abbreviated month name.",
long=" * %B, month name.",
long=" * %m, month, zero padded.",
long=" * %y, year without century, zero padded.",
long=" * %Y, year with century.",
long=" * %H, hour (24-hour clock) as a zero-padded number.",
long=" * %I, hour (12-hour clock) as a zero-padded number.",
long=" * %p, locale’s equivalent of either AM or PM.",
long=" * %M, minute as a zero-padded number.",
long=" * %S, second as a zero-padded number.",
long=" * %f, microsecond as a decimal number, zero-padded.",
long=" * %z, UTC offset in the form +HHMM or -HHMM.",
long=" * %Z, time zone name.",
long=" * %j, day of the year, zero-padded.",
long=" * %U, week number of the year (Sunday as first day of week).",
long=" * %W, week number of the year (Monday as first day of week).",
long=" * %c, default data and time representation of current locale.",
long=" * %x, default date representation of current locale.",
long=" * %X, default time representation of current locale.",
long=" * %%, a literal % character."
)]
struct Format {
    #[description("the format of the time.")]
    format: String,
}

fn format(context: CommandContext) -> CrushResult<()> {
    let time = context.this.time()?;
    let cfg: Format = Format::parse(context.arguments, &context.printer)?;
    context.output.send(Value::String(time.format(&cfg.format).to_string()))
}
