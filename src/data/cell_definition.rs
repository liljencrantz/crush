use crate::data::{Command, Cell, JobOutput, ListDefinition, CellType};
use crate::closure::ClosureDefinition;
use crate::job::{JobDefinition};
use std::path::Path;
use regex::Regex;
use crate::glob::Glob;
use chrono::{DateTime, Local};
use crate::env::Env;
use crate::printer::Printer;
use crate::errors::{JobError, mandate, error};
use crate::stream::streams;
use crate::commands::JobJoinHandle;

#[derive(Clone)]
pub enum CellDefinition {
    Text(Box<str>),
    Integer(i128),
    Time(DateTime<Local>),
    Field(Box<str>),
    Glob(Glob),
    Regex(Box<str>, Regex),
    Op(Box<str>),
    Command(Command),
    ClosureDefinition(ClosureDefinition),
    JobDefintion(JobDefinition),
    // During invocation, this will get replaced with an output
    File(Box<Path>),
    Variable(Box<str>),
    List(ListDefinition),
    ArrayVariable(Box<str>, Box<CellDefinition>),
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
                drop(first_output);
                let (last_output, last_input) = streams();
                let mut j = def.spawn_and_execute(&env, printer, first_input, last_output)?;

                let res = Cell::JobOutput(JobOutput { stream: last_input.initialize()? });
                dependencies.push(j);
                res
            }
            CellDefinition::ClosureDefinition(c) => Cell::ClosureDefinition(c.with_env(env)),
            CellDefinition::Variable(s) => (mandate(env.get(s.as_ref()), format!("Unknown variable {}", s.as_ref()).as_str())?).partial_clone()?,
            CellDefinition::List(l) => l.compile(dependencies, env, printer)?,
            CellDefinition::ArrayVariable(c, i) => {
                let cell = mandate(env.get(c.as_ref()), format!("Unknown variable {}", c.as_ref()).as_str())?;
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

    pub fn field(s: &str) -> CellDefinition {
        CellDefinition::Field(Box::from(s))
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
