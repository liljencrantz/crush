use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error_legacy, CrushResult, to_crush_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use chrono::{DateTime, Local};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::this::This;

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
            [],
        );
        res.declare(
            full("__sub__"),
            sub,
            false,
            "time - delta:duration",
            "Remove the specified delta from this time",
            None,
            Known(ValueType::Time),
            [],
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
long="After parsing the date, it will be converted to the local time zone.",
long="Date specifiers:",
long=" * %Y, year with century.",
long=" * %y, year without century, zero padded.",
long=" * %C, century, zero padded.",
long=" * %m, month, zero padded.",
long=" * %b, abbreviated month name.",
long=" * %h, abbreviated month name.",
long=" * %B, full month name.",
long=" * %d, day of month, zero padded.",
long=" * %e, day of month, space padded.",
long=" * %a, weekday as abbreviated name.",
long=" * %A, weekday as full name.",
long=" * %w, weekday as a number, where 0 is Sunday and 6 is Saturday.",
long=" * %u, weekday as a number, where 1 is Monday and 7 is Sunday.",
long=" * %U, week number of the year (Sunday as first day of week), zero padded.",
long=" * %W, week number of the year (Monday as first day of week), zero padded.",
long=" * %G, same to %Y but uses the year number in ISO 8601 week date.",
long=" * %g, same to %y but uses the year number in ISO 8601 week date.",
long=" * %V, same to %U but uses the year number in ISO 8601 week date.",
long=" * %j, day of the year, zero-padded.",
long=" * %D, month-day-year format. Same to %m/%d/%y.",
long=" * %x, month-day-year format. Same to %m/%d/%y.",
long=" * %F, year-month-day format (ISO 8601). Same to %Y-%m-%d.",
long=" * %v, day-month-year format. Same to %e-%b-%Y.",

long="Time specifiers:",
long=" * %H, hour (24-hour clock) as a zero-padded number.",
long=" * %k, hour (24-hour clock) as a space-padded number.",
long=" * %I, hour (12-hour clock) as a zero-padded number.",
long=" * %l, hour (12-hour clock) as a space-padded number.",
long=" * %P, locale’s equivalent of either am or pm.",
long=" * %p, locale’s equivalent of either AM or PM.",
long=" * %M, minute as a zero-padded number.",
long=" * %S, second as a zero-padded number.",
long=" * %f, fractional nanoseconds since last whole seconds, zero-padded.",
long=" * %R, hour-minute format. Same to %H:%M.",
long=" * %T, hour-minute-second format. Same to %H:%M:%S.",
long=" * %X, hour-minute-second format. Same to %H:%M:%S.",
long=" * %r, hour-minute-second format in 12-hour clocks. Same to %I:%M:%S %p.",

long="Time zone specifiers:",
long=" * %z, UTC offset in the form +HHMM or -HHMM.",
long=" * %Z, time zone name.",
long=" * %:z, a colon, followed by UTC offset in the form +HHMM or -HHMM.",

long="Special characters:",
long=" * %c, ctime date & time format. Same to %a %b %e %T %Y sans \\n.",
long=" * %+, ISO 8601 / RFC 3339 date & time format.",
long=" * %s, UNIX timestamp, the number of seconds since 1970-01-01 00:00 UTC.",
long=" * %t, a literal tab character.",
long=" * %n, a literal newline character.",
long=" * %%, a literal % character."
)]
struct Parse {
    #[description("the format of the time.")]
    format: String,
    #[description("the time string to parse.")]
    time: String,
}

fn parse(context: CommandContext) -> CrushResult<()> {
    let cfg: Parse = Parse::parse(context.arguments, &context.global_state.printer())?;
    let tm = to_crush_error(DateTime::parse_from_str(&cfg.time, &cfg.format))?;
    let dt = tm.with_timezone(&Local);
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

fn format(mut context: CommandContext) -> CrushResult<()> {
    let time = context.this.time()?;
    let cfg: Format = Format::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(Value::from(time.format(&cfg.format).to_string()))
}
