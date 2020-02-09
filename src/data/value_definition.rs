use std::path::Path;

use chrono::Local;
use regex::Regex;

use crate::{
    printer::Printer,
    glob::Glob,
    errors::{error, mandate, JobResult, argument_error, to_job_error},
    env::Env,
    data::{Value, ListDefinition},
    commands::JobJoinHandle,
    closure::Closure,
    stream::channels,
    stream::empty_channel,
    data::row::Struct
};
use std::time::Duration;
use crate::job::Job;

#[derive(Clone)]
#[derive(Debug)]
pub enum ValueDefinition {
    Text(Box<str>),
    Integer(i128),
    Time(Vec<ValueDefinition>),
    Duration(Vec<ValueDefinition>),
    Field(Vec<Box<str>>),
    Glob(Glob),
    Regex(Box<str>, Regex),
    Op(Box<str>),
    ClosureDefinition(Closure),
    JobDefinition(Job),
    MaterializedJobDefinition(Job),
    File(Box<Path>),
    Variable(Vec<Box<str>>),
    List(ListDefinition),
    Subscript(Box<ValueDefinition>, Box<ValueDefinition>),
}

fn to_duration(a: u64, t: &str) -> JobResult<Duration> {
    match t {
        "nanosecond" | "nanoseconds" => Ok(Duration::from_nanos(a)),
        "microsecond" | "microseconds" => Ok(Duration::from_micros(a)),
        "millisecond" | "milliseconds" => Ok(Duration::from_millis(a)),
        "second" | "seconds" => Ok(Duration::from_secs(a)),
        "minute" | "minutes" => Ok(Duration::from_secs(a*60)),
        "hour" | "hours" => Ok(Duration::from_secs(a*3600)),
        "day" | "days" => Ok(Duration::from_secs(a*3600*24)),
        "year" | "years" => Ok(Duration::from_secs(a*3600*24*365)),
        _ => Err(error("Invalid duration"))
    }
}

fn compile_duration_mode(cells: &Vec<ValueDefinition>, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> JobResult<Value> {
    let v: Vec<Value> = cells.iter()
        .map(|c| c.compile(dependencies, env, printer))
        .collect::<JobResult<Vec<Value>>>()?;
    let duration = match &v[..] {
        [Value::Integer(s)] => Duration::from_secs(*s as u64),
        [Value::Time(t1), Value::Text(operator), Value::Time(t2)] => if operator.as_ref() == "-" {
            to_job_error(t1.signed_duration_since(t2.clone()).to_std())?
        } else {
            return Err(error("Illegal duration"))
        },
        _ => if v.len() % 2 == 0 {
            let vec: Vec<Duration> = v.chunks(2)
                .map(|chunk| match (&chunk[0], &chunk[1]) {
                    (Value::Integer(a), Value::Text(t)) => to_duration(*a as u64, t.as_ref()),
                    _ => Err(argument_error("Unknown duration format"))
                })
                .collect::<JobResult<Vec<Duration>>>()?;
            vec.into_iter().sum::<Duration>()
        } else {
            return Err(error("Unknown duration format"))
        },
    };

    Ok(Value::Duration(duration))
}

fn compile_time_mode(cells: &Vec<ValueDefinition>, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> JobResult<Value> {
    let v: Vec<Value> = cells.iter()
        .map(|c | c.compile(dependencies, env, printer))
        .collect::<JobResult<Vec<Value>>>()?;
    let time = match &v[..] {
        [Value::Text(t)] => if t.as_ref() == "now" {Local::now()} else {return Err(error("Unknown time"))},
        _ => return Err(error("Unknown duration format")),
    };

    Ok(Value::Time(time))
}

impl ValueDefinition {
    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> JobResult<Value> {
        Ok(match self {
            ValueDefinition::Text(v) => Value::Text(v.clone()),
            ValueDefinition::Integer(v) => Value::Integer(v.clone()),
            ValueDefinition::Time(v) => compile_time_mode(v, dependencies, env, printer)?,
            ValueDefinition::Duration(c) => compile_duration_mode(c, dependencies, env, printer)?,
            ValueDefinition::Field(v) => Value::Field(v.clone()),
            ValueDefinition::Glob(v) => Value::Glob(v.clone()),
            ValueDefinition::Regex(v, r) => Value::Regex(v.clone(), r.clone()),
            ValueDefinition::Op(v) => Value::Op(v.clone()),
            ValueDefinition::File(v) => Value::File(v.clone()),
            ValueDefinition::JobDefinition(def) => {
                let first_input = empty_channel();
                let (last_output, last_input) = channels();
                let j = def.spawn_and_execute(&env, printer, first_input, last_output)?;
                dependencies.push(j);
                last_input.recv()?
            }
            ValueDefinition::MaterializedJobDefinition(def) => {
                let first_input = empty_channel();
                let (last_output, last_input) = channels();
                let j = def.spawn_and_execute(&env, printer, first_input, last_output)?;
                dependencies.push(j);
                last_input.recv()?.materialize()
            }
            ValueDefinition::ClosureDefinition(c) => Value::Closure(c.with_env(env)),
            ValueDefinition::Variable(s) => (
                mandate(
                    env.get(s),
                    format!("Unknown variable {}", self.to_string()).as_str())?),
            ValueDefinition::List(l) => l.compile(dependencies, env, printer)?,
            ValueDefinition::Subscript(c, i) => {
                match (c.compile(dependencies, env, printer), i.compile(dependencies, env, printer)) {
                    (Ok(Value::List(list)), Ok(Value::Integer(idx))) =>
                        list.get(idx as usize)?,
                    (Ok(Value::Dict(dict)), Ok(c)) =>
                        mandate(dict.get(&c), "Invalid subscript")?,
                    (Ok(Value::Env(env)), Ok(Value::Text(name))) =>
                        mandate(env.get_str(name.as_ref()), "Invalid subscript")?,
                    (Ok(Value::Struct(row)), Ok(Value::Text(col))) =>
                        mandate(row.get(col.as_ref()), "Invalid subscript")?,
                    (Ok(Value::Stream(o)), Ok(Value::Integer(idx))) => {
                        Value::Struct(o.get(idx)?.into_struct(o.stream.get_type()))
                    }
                    _ => return Err(error("Expected a list variable")),
                }
            }
        })
    }

    pub fn text(s: &str) -> ValueDefinition {
        ValueDefinition::Text(Box::from(s))
    }

    pub fn op(s: &str) -> ValueDefinition {
        ValueDefinition::Op(Box::from(s))
    }

    pub fn regex(s: &str, r: Regex) -> ValueDefinition {
        ValueDefinition::Regex(Box::from(s), r)
    }
}

impl ToString for ValueDefinition {
    fn to_string(&self) -> String {
        match self {
            ValueDefinition::Text(t) => t.to_string(),
            ValueDefinition::Integer(i) => format!("{}", i),
            ValueDefinition::Variable(v) => format!("${}", v.join(".")),
            _ => panic!("Unimplementd conversion"),
        }
    }
}
