use std::path::Path;

use chrono::Local;
use regex::Regex;

use crate::{
    printer::Printer,
    glob::Glob,
    errors::{error, mandate, CrushResult, argument_error, to_job_error},
    env::Env,
    data::{Value},
    lib::JobJoinHandle,
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
    Value(Value),
    ClosureDefinition(Closure),
    JobDefinition(Job),
    MaterializedJobDefinition(Job),
    Variable(Vec<Box<str>>),
    Subscript(Box<ValueDefinition>, Box<ValueDefinition>),
}

impl ValueDefinition {
    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> CrushResult<Value> {
        Ok(match self {
            ValueDefinition::Value(v) => v.clone(),
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
                    _ => return error("Value can't be subscripted"),
                }
            }
        })
    }

    pub fn text(s: &str) -> ValueDefinition {
        ValueDefinition::Value(Value::Text(Box::from(s)))
    }

    pub fn field(s: Vec<Box<str>>) -> ValueDefinition {
        ValueDefinition::Value(Value::Field(s))
    }

    pub fn glob(s: &str) -> ValueDefinition {
        ValueDefinition::Value(Value::Glob(Glob::new(s)))
    }

    pub fn integer(i: i128) -> ValueDefinition {
        ValueDefinition::Value(Value::Integer(i))
    }

    pub fn float(f: f64) -> ValueDefinition {
        ValueDefinition::Value(Value::Float(f))
    }

    pub fn regex(s: &str, r: Regex) -> ValueDefinition {
        ValueDefinition::Value(Value::Regex(Box::from(s), r))
    }
}

impl ToString for ValueDefinition {
    fn to_string(&self) -> String {
        match self {
            ValueDefinition::Value(v) => v.to_string(),
            ValueDefinition::Variable(v) => format!("${}", v.join(".")),
            _ => panic!("Unimplementd conversion"),
        }
    }
}
