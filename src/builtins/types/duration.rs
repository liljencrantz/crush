use crate::lang::command::Command;
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::command::TypeMap;
use crate::lang::errors::{CrushResult, argument_error_legacy};
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use chrono::Duration;
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "duration", name]
}

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        res.declare(
            full("__add__"),
            add,
            false,
            "duration + (delta:duration | time:time)",
            "Add the specified delta or time to this duration.",
            None,
            Unknown,
            [],
        );
        res.declare(
            full("__sub__"),
            sub,
            false,
            "duration - delta:duration",
            "Remove the specified delta from this duration.",
            None,
            Known(ValueType::Duration),
            [],
        );
        res.declare(
            full("__mul__"),
            mul,
            false,
            "duration * factor:integer",
            "Multiply this duration by the specified factor.",
            None,
            Known(ValueType::Duration),
            [],
        );
        res.declare(
            full("__div__"),
            div,
            false,
            "duration / divisor:integer",
            "Divide this duration by the specified divisor.",
            None,
            Known(ValueType::Duration),
            [],
        );
        Of::declare_method(&mut res);
        Milliseconds::declare_method(&mut res);
        Seconds::declare_method(&mut res);
        Minutes::declare_method(&mut res);
        Hours::declare_method(&mut res);
        Days::declare_method(&mut res);
        NanosecondsPart::declare_method(&mut res);
        Neg::declare_method(&mut res);

        res
    })
}

binary_op!(
    add,
    duration,
    Duration,
    Duration,
    |a, b| a + b,
    Time,
    Time,
    |a, b| b + a
);
binary_op!(sub, duration, Duration, Duration, |a, b| a - b);
binary_op!(mul, duration, Integer, Duration, |a, b| a * (b as i32));
binary_op!(div, duration, Integer, Duration, |a, b| a / (b as i32));

#[allow(unused)]
fn to_duration(a: i64, t: &str) -> CrushResult<chrono::Duration> {
    match t {
        "nanosecond" | "nanoseconds" => Ok(Duration::nanoseconds(a)),
        "microsecond" | "microseconds" => Ok(Duration::microseconds(a)),
        "millisecond" | "milliseconds" => Ok(Duration::milliseconds(a)),
        "second" | "seconds" => Ok(Duration::seconds(a)),
        "minute" | "minutes" => Ok(Duration::seconds(a * 60)),
        "hour" | "hours" => Ok(Duration::seconds(a * 3600)),
        "day" | "days" => Ok(Duration::seconds(a * 3600 * 24)),
        _ => argument_error_legacy("Invalid duration"),
    }
}

#[signature(
    types.duration.of,
    can_block = false,
    output = Known(ValueType::Duration),
    short = "Create a new duration.",
    long = "Durations are stored as a time span in number of seconds. Because of leap seconds and daylight saving time, adding for example exactly one day to a `time` value will not always do what you might think, if a leap second or a daylight savings time changeover happened in the interim.",
    example = "duration:of minutes=1",
)]
struct Of {
    #[description("the number of nanoseconds in the duration.")]
    #[default(0)]
    nanoseconds: i64,
    #[description("the number of microseconds in the duration.")]
    #[default(0)]
    microseconds: i64,
    #[description("the number of milliseconds in the duration.")]
    #[default(0)]
    milliseconds: i64,
    #[description("the number of seconds in the duration.")]
    #[default(0)]
    seconds: i64,
    #[description("the number of minutes in the duration.")]
    #[default(0)]
    minutes: i64,
    #[description("the number of hours in the duration.")]
    #[default(0)]
    hours: i64,
    #[description("the number of days in the duration. This is internally represented as the number of seconds in a standard day.")]
    #[default(0)]
    days: i64,
}

fn of(context: CommandContext) -> CrushResult<()> {
    let cfg: Of = Of::parse(context.arguments, &context.global_state.printer())?;

    let res = Duration::nanoseconds(cfg.nanoseconds)
        + Duration::microseconds(cfg.microseconds)
        + Duration::milliseconds(cfg.milliseconds)
        + Duration::seconds(cfg.seconds)
        + Duration::minutes(cfg.minutes)
        + Duration::hours(cfg.hours)
        + Duration::days(cfg.days);
    context.output.send(Value::Duration(res))
}

#[signature(
    types.duration.seconds,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the number of seconds in this duration, rounded towards zero.",
)]
struct Seconds {
}

fn seconds(mut context: CommandContext) -> CrushResult<()> {
    Seconds::parse(context.remove_arguments(), &context.global_state.printer())?;
    let this = context.this.duration()?;
    context.output.send(Value::Integer(this.num_seconds() as i128))
}

#[signature(
    types.duration.minutes,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the number of minutes in this duration, rounded towards zero.",
)]
struct Minutes {
}

fn minutes(mut context: CommandContext) -> CrushResult<()> {
    Minutes::parse(context.remove_arguments(), &context.global_state.printer())?;
    let this = context.this.duration()?;
    context.output.send(Value::Integer((this.num_seconds() / 60) as i128))
}

#[signature(
    types.duration.hours,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the number of minutes in this duration, rounded towards zero.",
)]
struct Hours {
}

fn hours(mut context: CommandContext) -> CrushResult<()> {
    Hours::parse(context.remove_arguments(), &context.global_state.printer())?;
    let this = context.this.duration()?;
    context.output.send(Value::Integer((this.num_seconds() / (60 * 60)) as i128))
}

#[signature(
    types.duration.days,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the number of minutes in this duration, rounded towards zero.",
)]
struct Days {
}

fn days(mut context: CommandContext) -> CrushResult<()> {
    Days::parse(context.remove_arguments(), &context.global_state.printer())?;
    let this = context.this.duration()?;
    context.output.send(Value::Integer((this.num_seconds() / (60 * 60 * 24)) as i128))
}

#[signature(
    types.duration.milliseconds,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the number of milliseconds in this duration, rounded towards zero.",
)]
struct Milliseconds {
}

fn milliseconds(mut context: CommandContext) -> CrushResult<()> {
    Days::parse(context.remove_arguments(), &context.global_state.printer())?;
    let this = context.this.duration()?;
    context.output.send(Value::Integer(this.num_milliseconds() as i128))
}

#[signature(
    types.duration.nanoseconds_part,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the nanosecond part of this duration.",
)]
struct NanosecondsPart {
}

fn nanoseconds_part(mut context: CommandContext) -> CrushResult<()> {
    Days::parse(context.remove_arguments(), &context.global_state.printer())?;
    let this = context.this.duration()?;
    context.output.send(Value::Integer(this.subsec_nanos() as i128))
}

#[signature(
    types.duration.__neg__,
    can_block = false,
    output = Known(ValueType::Duration),
    short = "Negate this duration.",
)]
struct Neg {
}

fn __neg__(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Duration(-context.this.duration()?))
}
