use std::path::Path;

use chrono::Local;
use regex::Regex;

use crate::{
    printer::Printer,
    glob::Glob,
    errors::{error, mandate, CrushResult, argument_error, to_job_error},
    env::Env,
    data::{Value},
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
    Field(Vec<Box<str>>),
    Glob(Glob),
    Regex(Box<str>, Regex),
    Op(Box<str>),
    ClosureDefinition(Closure),
    JobDefinition(Job),
    MaterializedJobDefinition(Job),
    File(Box<Path>),
    Variable(Vec<Box<str>>),
    Subscript(Box<ValueDefinition>, Box<ValueDefinition>),
}

impl ValueDefinition {
    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> CrushResult<Value> {
        Ok(match self {
            ValueDefinition::Text(v) => Value::Text(v.clone()),
            ValueDefinition::Integer(v) => Value::Integer(v.clone()),
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
