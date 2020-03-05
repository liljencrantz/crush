use std::path::Path;

use chrono::Local;
use regex::Regex;

use crate::{
    lang::printer::Printer,
    util::glob::Glob,
    lang::errors::{error, mandate, CrushResult, argument_error, to_crush_error},
    lang::scope::Scope,
    lang::value::Value,
    lang::job::JobJoinHandle,
    lang::command::Closure,
    lang::stream::channels,
    lang::stream::empty_channel,
    lang::r#struct::Struct
};
use std::time::Duration;
use crate::lang::{job::Job, argument::ArgumentDefinition, command::CrushCommand};

#[derive(Clone)]
#[derive(Debug)]
pub enum ValueDefinition {
    Value(Value),
    ClosureDefinition(Vec<Job>),
    JobDefinition(Job),
    Variable(Vec<Box<str>>),
    Subscript(Box<ValueDefinition>, Box<ValueDefinition>),
}

impl ValueDefinition {
    pub fn can_block(&self, arg: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
        match self {
            ValueDefinition::JobDefinition(j) => j.can_block(arg, env),
            ValueDefinition::Subscript(inner1, inner2) => inner1.can_block(arg, env) || inner2.can_block(arg, env),
            _ => false,
        }
    }

    pub fn can_block_when_called(&self, arg: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
        match self {
            ValueDefinition::ClosureDefinition(c) => {
                if (c.len() == 1) {
                    Closure::new(c.clone(), env).can_block(arg, env)
                } else {
                    true
                }
            }
            ValueDefinition::JobDefinition(j) => j.can_block(arg, env),
            ValueDefinition::Subscript(inner1, inner2) => inner1.can_block(arg, env) || inner2.can_block(arg, env),
            _ => false,
        }
    }

    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Scope, printer: &Printer) -> CrushResult<Value> {
        Ok(match self {
            ValueDefinition::Value(v) => v.clone(),
            ValueDefinition::JobDefinition(def) => {
                let first_input = empty_channel();
                let (last_output, last_input) = channels();
                let j = def.invoke(&env, printer, first_input, last_output)?;
                dependencies.push(j);
                last_input.recv()?
            }
            ValueDefinition::ClosureDefinition(c) => Value::Closure(Closure::new(c.clone(), env)),
            ValueDefinition::Variable(s) =>
                mandate(
                    env.get(s),
                    format!("Unknown variable {}", self.to_string()).as_str())?,
            ValueDefinition::Subscript(c, i) =>
                match (c.compile(dependencies, env, printer), i.compile(dependencies, env, printer)) {
                    (Ok(Value::List(list)), Ok(Value::Integer(idx))) =>
                        list.get(idx as usize)?,
                    (Ok(Value::Dict(dict)), Ok(c)) =>
                        mandate(dict.get(&c), "Invalid subscript")?,
                    (Ok(Value::Scope(env)), Ok(Value::Text(name))) =>
                        mandate(env.get_str(name.as_ref()), "Invalid subscript")?,
                    (Ok(Value::Struct(row)), Ok(Value::Text(col))) =>
                        mandate(row.get(col.as_ref()), "Invalid subscript")?,
                    (Ok(Value::TableStream(o)), Ok(Value::Integer(idx))) => {
                        Value::Struct(o.get(idx)?.into_struct(o.stream.types()))
                    }
                    _ => return error("Value can't be subscripted"),
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
