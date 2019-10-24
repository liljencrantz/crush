use crate::data::{Command, Cell, JobOutput};
use crate::closure::ClosureDefinition;
use crate::job::{JobDefinition, Job};
use std::path::Path;
use regex::Regex;
use crate::glob::Glob;
use chrono::{DateTime, Local};
use crate::env::Env;
use crate::printer::Printer;
use crate::errors::{JobError, mandate};
use crate::stream::streams;

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
}

impl CellDefinition {
    pub fn compile(self, dependencies: &mut Vec<Job>, env: &Env, printer: &Printer) -> Result<Cell, JobError> {
        Ok(match self {
            CellDefinition::Text(v) => Cell::Text(v),
            CellDefinition::Integer(v) => Cell::Integer(v),
            CellDefinition::Time(v) => Cell::Time(v),
            CellDefinition::Field(v) => Cell::Field(v),
            CellDefinition::Glob(v) => Cell::Glob(v),
            CellDefinition::Regex(v, r) => Cell::Regex(v, r),
            CellDefinition::Op(v) => Cell::Op(v),
            CellDefinition::Command(v) => Cell::Command(v),
            CellDefinition::File(v) => Cell::File(v),
            //CellDefinition::Rows(r) => Cell::Rows(r),
            CellDefinition::JobDefintion(def) => {
                let (first_output, first_input) = streams();
                drop(first_output);
                let (last_output, last_input) = streams();
                let mut j = def.compile(&env, printer, &vec![], first_input, last_output)?;

                let res = Cell::JobOutput(JobOutput { types: j.get_output_type().clone(), stream: last_input });
                dependencies.push(j);
                res
            }
            CellDefinition::ClosureDefinition(c) => Cell::ClosureDefinition(c.clone()),
            CellDefinition::Variable(s) => (mandate(env.get(s.as_ref()), format!("Unknown variable {}", s.as_ref()).as_str())?).partial_clone()?,
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
