use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Unknown;
use crate::lang::command::TypeMap;
use crate::lang::errors::CrushResult;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use chrono::{DateTime, Duration, Local};
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "time", name]
}

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Add::declare_method(&mut res);
        res.declare(
            full("__sub__"),
            __sub__,
            false,
            "time - duration | time",
            "Remove the specified duration from this time to produce an earlier time, or calculate the difference between two points in time.",
            None,
            Unknown,
            [],
        );
        Now::declare_method(&mut res);
        Parse::declare_method(&mut res);
        Format::declare_method(&mut res);
        res
    })
}

#[signature(
    types.integer.__add__,
    can_block = false,
    output = Known(ValueType::Time),
    short = "Add the specified delta to this time",
)]
#[allow(unused)]
struct Add {
    #[description("the number to add")]
    term: Duration,
}

binary_op!(__add__, time, Duration, Time, |a, b| a + b);

binary_op!(
    __sub__,
    time,
    Duration,
    Time,
    |a, b| a - b,
    Time,
    Duration,
    |a, b| a - b
);

#[signature(
    types.time.now,
    can_block = false,
    output = Known(ValueType::Time),
    short = "The current point in time.",
    example = "time:now"
)]
struct Now {}

fn now(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::Time(Local::now()))
}

#[signature(
    types.time.parse,
    can_block = false,
    output = Known(ValueType::Time),
    short = "Parse a time string using a strptime-style pattern string",
    long = "After parsing the date, it will be converted to the local time zone.",
    long = "Date specifiers:",
    long = " * `%Y` year with century.",
    long = " * `%y` year without century, zero padded.",
    long = " * `%C` century, zero padded.",
    long = " * `%m` month, zero padded.",
    long = " * `%b` abbreviated month name.",
    long = " * `%h` abbreviated month name.",
    long = " * `%B` full month name.",
    long = " * `%d` day of month, zero padded.",
    long = " * `%e` day of month, space padded.",
    long = " * `%a` weekday as abbreviated name.",
    long = " * `%A` weekday as full name.",
    long = " * `%w` weekday as a number, where 0 is Sunday and 6 is Saturday.",
    long = " * `%u` weekday as a number, where 1 is Monday and 7 is Sunday.",
    long = " * `%U` week number of the year (Sunday as first day of week), zero padded.",
    long = " * `%W` week number of the year (Monday as first day of week), zero padded.",
    long = " * `%G` same to %Y but uses the year number in ISO 8601 week date.",
    long = " * `%g` same to %y but uses the year number in ISO 8601 week date.",
    long = " * `%V` same to %U but uses the year number in ISO 8601 week date.",
    long = " * `%j` day of the year, zero-padded.",
    long = " * `%D` month-day-year format. Same to %m/%d/%y.",
    long = " * `%x` month-day-year format. Same to %m/%d/%y.",
    long = " * `%F` year-month-day format (ISO 8601). Same to %Y-%m-%d.",
    long = " * `%v` day-month-year format. Same to %e-%b-%Y.",
    long = "Time specifiers:",
    long = " * `%H` hour (24-hour clock) as a zero-padded number.",
    long = " * `%k` hour (24-hour clock) as a space-padded number.",
    long = " * `%I` hour (12-hour clock) as a zero-padded number.",
    long = " * `%l` hour (12-hour clock) as a space-padded number.",
    long = " * `%P` locale’s equivalent of either am or pm.",
    long = " * `%p` locale’s equivalent of either AM or PM.",
    long = " * `%M` minute as a zero-padded number.",
    long = " * `%S` second as a zero-padded number.",
    long = " * `%f` fractional nanoseconds since last whole seconds, zero-padded.",
    long = " * `%R` hour-minute format. Same to %H:%M.",
    long = " * `%T` hour-minute-second format. Same to %H:%M:%S.",
    long = " * `%X` hour-minute-second format. Same to %H:%M:%S.",
    long = " * `%r` hour-minute-second format in 12-hour clocks. Same to %I:%M:%S %p.",
    long = "Time zone specifiers:",
    long = " * `%z` UTC offset in the form +HHMM or -HHMM.",
    long = " * `%Z` time zone name.",
    long = " * `%:z` a colon, followed by UTC offset in the form +HHMM or -HHMM.",
    long = "Special characters:",
    long = " * `%c` ctime date & time format. Same to %a %b %e %T %Y sans \\n.",
    long = " * `%+` ISO 8601 / RFC 3339 date & time format.",
    long = " * `%s` UNIX timestamp, the number of seconds since 1970-01-01 00:00 UTC.",
    long = " * `%t` a literal tab character.",
    long = " * `%n` a literal newline character.",
    long = " * `%%` a literal % character.",
    example = "time:parse format=\"%s\" time=\"1234567890\"",
)]
struct Parse {
    #[description("the format of the time.")]
    format: String,
    #[description("the time string to parse.")]
    time: String,
}

fn parse(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Parse = Parse::parse(context.remove_arguments(), &context.global_state.printer())?;
    let tm = DateTime::parse_from_str(&cfg.time, &cfg.format)?;
    let dt = tm.with_timezone(&Local);
    context.output.send(Value::Time(dt))
}

#[signature(
    types.time.format,
    can_block = false,
    output = Known(ValueType::String),
    short = "Format this time using a strftime-style pattern string",
    long = "Date specifiers:",
    long = " * `%Y` year with century.",
    long = " * `%y` year without century, zero padded.",
    long = " * `%C` century, zero padded.",
    long = " * `%m` month, zero padded.",
    long = " * `%b` abbreviated month name.",
    long = " * `%h` abbreviated month name.",
    long = " * `%B` full month name.",
    long = " * `%d` day of month, zero padded.",
    long = " * `%e` day of month, space padded.",
    long = " * `%a` weekday as abbreviated name.",
    long = " * `%A` weekday as full name.",
    long = " * `%w` weekday as a number, where 0 is Sunday and 6 is Saturday.",
    long = " * `%u` weekday as a number, where 1 is Monday and 7 is Sunday.",
    long = " * `%U` week number of the year (Sunday as first day of week), zero padded.",
    long = " * `%W` week number of the year (Monday as first day of week), zero padded.",
    long = " * `%G` same to %Y but uses the year number in ISO 8601 week date.",
    long = " * `%g` same to %y but uses the year number in ISO 8601 week date.",
    long = " * `%V` same to %U but uses the year number in ISO 8601 week date.",
    long = " * `%j` day of the year, zero-padded.",
    long = " * `%D` month-day-year format. Same to %m/%d/%y.",
    long = " * `%x` month-day-year format. Same to %m/%d/%y.",
    long = " * `%F` year-month-day format (ISO 8601). Same to %Y-%m-%d.",
    long = " * `%v` day-month-year format. Same to %e-%b-%Y.",
    long = "Time specifiers:",
    long = " * `%H` hour (24-hour clock) as a zero-padded number.",
    long = " * `%k` hour (24-hour clock) as a space-padded number.",
    long = " * `%I` hour (12-hour clock) as a zero-padded number.",
    long = " * `%l` hour (12-hour clock) as a space-padded number.",
    long = " * `%P` locale’s equivalent of either am or pm.",
    long = " * `%p` locale’s equivalent of either AM or PM.",
    long = " * `%M` minute as a zero-padded number.",
    long = " * `%S` second as a zero-padded number.",
    long = " * `%f` fractional nanoseconds since last whole seconds, zero-padded.",
    long = " * `%R` hour-minute format. Same to %H:%M.",
    long = " * `%T` hour-minute-second format. Same to %H:%M:%S.",
    long = " * `%X` hour-minute-second format. Same to %H:%M:%S.",
    long = " * `%r` hour-minute-second format in 12-hour clocks. Same to %I:%M:%S %p.",
    long = "Time zone specifiers:",
    long = " * `%z` UTC offset in the form +HHMM or -HHMM.",
    long = " * `%Z` time zone name.",
    long = " * `%:z` a colon, followed by UTC offset in the form +HHMM or -HHMM.",
    long = "Special characters:",
    long = " * `%c` ctime date & time format. Same to %a %b %e %T %Y sans \\n.",
    long = " * `%+` ISO 8601 / RFC 3339 date & time format.",
    long = " * `%s` UNIX timestamp, the number of seconds since 1970-01-01 00:00 UTC.",
    long = " * `%t` a literal tab character.",
    long = " * `%n` a literal newline character.",
    long = " * `%%` a literal % character.",
    example = "time:now:format \"%s\"",
)]
struct Format {
    #[description("the format of the time.")]
    format: String,
}

fn format(mut context: CommandContext) -> CrushResult<()> {
    let time = context.this.time()?;
    let cfg: Format = Format::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::from(time.format(&cfg.format).to_string()))
}
