use std::path::Path;

use chrono::{DateTime, Local};
use regex::Regex;

use crate::closure::ClosureDefinition;
use crate::commands::JobJoinHandle;
use crate::data::{Cell, Command, JobOutput, ListDefinition};
use crate::env::Env;
use crate::errors::{error, JobError, mandate};
use crate::glob::Glob;
use crate::job::JobDefinition;
use crate::printer::Printer;
use crate::stream::streams;

#[derive(Clone)]
pub enum CellDefinition {
    Text(Box<str>),
    Integer(i128),
    Time(DateTime<Local>),
    Field(Vec<Box<str>>),
    Glob(Glob),
    Regex(Box<str>, Regex),
    Op(Box<str>),
    Command(Command),
    ClosureDefinition(ClosureDefinition),
    JobDefintion(JobDefinition),
    // During invocation, this will get replaced with an output
    File(Box<Path>),
    Variable(Vec<Box<str>>),
    List(ListDefinition),
    ArrayVariable(Vec<Box<str>>, Box<CellDefinition>),
}

impl CellDefinition {
    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> Result<Cell, JobError> {
        Ok(match self {
            CellDefinition::Text(v) => Cell::Text(v.clone()),
            CellDefinition::Integer(v) => Cell::Integer(v.clone()),
            CellDefinition::Time(v) => Cell::Time(v.clone()),
            CellDefinition::Field(v) => Cell::Field(v.clone()),
            CellDefinition::Glob(v) => Cell::Glob(v.clone()),
            CellDefinition::Regex(v, r) => Cell::Regex(v.clone(), r.clone()),
            CellDefinition::Op(v) => Cell::Op(v.clone()),
            CellDefinition::Command(v) => Cell::Command(v.clone()),
            CellDefinition::File(v) => Cell::File(v.clone()),
            //CellDefinition::Rows(r) => Cell::Rows(r),
            CellDefinition::JobDefintion(def) => {
                let (first_output, first_input) = streams();
                first_output.initialize(vec![])?;
                let (last_output, last_input) = streams();
                let mut j = def.spawn_and_execute(&env, printer, first_input, last_output)?;

                let res = Cell::JobOutput(JobOutput { stream: last_input.initialize()? });
                dependencies.push(j);
                res
            }
            CellDefinition::ClosureDefinition(c) => Cell::ClosureDefinition(c.with_env(env)),
            CellDefinition::Variable(s) => (
                mandate(
                    env.get(s),
                    format!("Unknown variable").as_str())?).partial_clone()?,
            CellDefinition::List(l) => l.compile(dependencies, env, printer)?,
            CellDefinition::ArrayVariable(c, i) => {
                let cell = mandate(env.get(c), format!("Unknown variable").as_str())?;
                if let Cell::List(arr) = cell {
                    let idx_cell = i.compile(dependencies, env, printer)?;
                    if let Cell::Integer(idx) = idx_cell {
                        return arr.get(idx as usize);
                    } else {
                        return Err(error("Expected an index"));
                    }
                } else {
                    return Err(error("Expected a list variable"));
                }
            }
        })
    }

    pub fn file(s: &str) -> CellDefinition {
        CellDefinition::File(Box::from(Path::new(s)))
    }

    pub fn text(s: &str) -> CellDefinition {
        CellDefinition::Text(Box::from(s))
    }

    pub fn op(s: &str) -> CellDefinition {
        CellDefinition::Op(Box::from(s))
    }

    pub fn regex(s: &str, r: Regex) -> CellDefinition {
        CellDefinition::Regex(Box::from(s), r)
    }
}

impl PartialEq for CellDefinition {
    fn eq(&self, other: &Self) -> bool {
        unimplemented!()
    }
}
