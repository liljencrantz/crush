use crate::commands::CompileContext;
use crate::errors::{CrushResult, argument_error, to_job_error, CrushError};
use crate::data::{Value, Command, ValueType};
use crate::env::Env;
use chrono::{Local, Duration, DateTime, NaiveDateTime, Datelike, Timelike};
use std::ops::Add;
use chrono::format::Parsed;
use crate::commands::parse_util::single_argument_text;
use core::num::FpCategory::Nan;
use time::{strptime, Tm};
use std::cmp::max;

fn now(mut context: CompileContext) -> CrushResult<()> {
    context.output.send(Value::Time(Local::now()))
}

fn parse(mut context: CompileContext) -> CrushResult<()> {
    let mut tm: Option<Box<str>> = None;
    let mut fmt: Option<Box<str>> = None;

    for arg in context.arguments.drain(..) {
        match (arg.name.as_deref().unwrap_or(""), arg.value) {
            ("format", Value::Text(s)) => fmt = Some(s),
            ("date", Value::Text(s)) => tm = Some(s),
            _ => return Err(argument_error("Invalid argument")),
        }
    }

    match (tm, fmt) {
        (Some(t), Some(f)) => {
            let tm = to_job_error(strptime(t.as_ref(), f.as_ref()))?;
            let mut dt = Local::now()
                .with_year(tm.tm_year + 1900).unwrap()
                .with_month0(tm.tm_mon as u32).unwrap()
                .with_day(max(tm.tm_mday as u32, 1)).unwrap()
                .with_hour(tm.tm_hour as u32).unwrap()
                .with_minute(tm.tm_min as u32).unwrap()
                .with_second(tm.tm_sec as u32).unwrap()
                .with_nanosecond(tm.tm_nsec as u32).unwrap();
            context.output.send(Value::Time(dt))
        }
        _ => Err(argument_error("Must specify both time and format")),
    }
}

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
        _ => Err(argument_error("Invalid duration"))
    }
}

fn duration(mut context: CompileContext) -> CrushResult<()> {
    let v: Vec<Value> = context.arguments.drain(..)
        .map(|a| a.value)
        .collect::<Vec<Value>>();
    let duration = match &v[..] {
        [Value::Integer(s)] => Duration::seconds(*s as i64),
        [Value::Time(t1), Value::Text(operator), Value::Time(t2)] => if operator.as_ref() == "-" {
            t1.signed_duration_since(t2.clone())
        } else {
            return Err(argument_error("Illegal duration"));
        },
        _ => {
            if v.len() % 2 == 0 {
                let vec = v.chunks(2)
                    .map(|chunks| match (&chunks[0], &chunks[1]) {
                        (Value::Integer(a), Value::Text(t)) => to_duration(*a as i64, t.as_ref()),
                        _ => Err(argument_error("Unknown duration format")),
                    })
                    .collect::<CrushResult<Vec<Duration>>>()?;
                let mut res = Duration::seconds(0);
                vec.iter()
                    .for_each(|d| {
                        res = res + d.clone();
                    });
                res
            } else {
                return Err(argument_error("Unknown duration format"));
            }
        }
    };
    context.output.send(Value::Duration(duration))
}

pub fn declare(root: &Env) -> CrushResult<()> {
    let env = root.create_namespace("time")?;
    env.declare_str("now", Value::Command(Command::new(now)))?;
    env.declare_str("parse", Value::Command(Command::new(parse)))?;
    env.declare_str("duration", Value::Command(Command::new(duration)))?;
    Ok(())
}
