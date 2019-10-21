use crate::glob::Glob;
use std::cmp::Ordering;
use std::hash::Hasher;
use crate::data::{Output, CellDataType, Command, Row};
use crate::data::rows::Rows;
use crate::errors::{error, JobError, to_job_error, mandate};
use std::path::Path;
use regex::Regex;
use chrono::{DateTime, Local};
use crate::env::{get_cwd, Env};
use crate::job::{JobDefinition, Job};
use crate::closure::{Closure, ClosureDefinition};
use crate::stream::streams;
use crate::printer::Printer;

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
    Closure(ClosureDefinition),
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

                let res = Cell::Output(Output { types: j.get_output_type().clone(), stream: last_input });
                dependencies.push(j);
                res
            }
            CellDefinition::Closure(c) => Cell::Closure(c.compile(env)),
            CellDefinition::Variable(s) => (mandate(env.get(s.as_ref()))?).partial_clone()?,
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

pub enum Cell {
    Text(Box<str>),
    Integer(i128),
    Time(DateTime<Local>),
    Field(Box<str>),
    Glob(Glob),
    Regex(Box<str>, Regex),
    Op(Box<str>),
    Command(Command),
    // This is a cell that contains a crush builtin command
    Closure(Closure),
    Output(Output),
    File(Box<Path>),
    Rows(Rows),
}


impl Cell {
    fn to_rows(s: &Output) -> Cell {
        let mut rows: Vec<Row> = Vec::new();
        loop {
            match s.stream.recv() {
                Ok(row) => {
                    rows.push(row.concrete());
                }
                Err(_) => break,
            }
        }
        return Cell::Rows(Rows { types: s.types.clone(), rows });
    }

    pub fn to_string(&self) -> String {
        return match self {
            Cell::Text(val) => val.to_string(),
            Cell::Integer(val) => val.to_string(),
            Cell::Time(val) => val.format("%Y-%m-%d %H:%M:%S %z").to_string(),
            Cell::Field(val) => format!(r"%{}", val),
            Cell::Glob(val) => format!("*{{{}}}", val.to_string()),
            Cell::Regex(val, _) => format!("r{{{}}}", val),
            Cell::Op(val) => val.to_string(),
            Cell::Command(_) => "Command".to_string(),
            Cell::File(val) => val.to_str().unwrap_or("<Broken file>").to_string(),
            Cell::Rows(_) => "<Table>".to_string(),
            Cell::Closure(_) => "<Closure>".to_string(),
            Cell::Output(_) => "<Table>".to_string(),
        };
    }

    pub fn alignment(&self) -> Alignment {
        return match self {
            Cell::Integer(_) => Alignment::Right,
            _ => Alignment::Left,
        };
    }

    pub fn file(s: &str) -> Cell {
        Cell::File(Box::from(Path::new(s)))
    }

    pub fn text(s: &str) -> Cell {
        Cell::Text(Box::from(s))
    }

    pub fn field(s: &str) -> Cell {
        Cell::Field(Box::from(s))
    }

    pub fn op(s: &str) -> Cell {
        Cell::Op(Box::from(s))
    }

    pub fn regex(s: &str, r: Regex) -> Cell {
        Cell::Regex(Box::from(s), r)
    }

    pub fn cell_data_type(&self) -> CellDataType {
        return match self {
            Cell::Text(_) => CellDataType::Text,
            Cell::Integer(_) => CellDataType::Integer,
            Cell::Time(_) => CellDataType::Time,
            Cell::Field(_) => CellDataType::Field,
            Cell::Glob(_) => CellDataType::Glob,
            Cell::Regex(_, _) => CellDataType::Regex,
            Cell::Op(_) => CellDataType::Op,
            Cell::Command(_) => CellDataType::Command,
            Cell::File(_) => CellDataType::File,
            Cell::Output(o) => CellDataType::Output(o.types.clone()),
            Cell::Rows(r) => CellDataType::Rows(r.types.clone()),
            Cell::Closure(c) => CellDataType::Closure,
        };
    }

    pub fn concrete(self) -> Cell {
        return match self {
            Cell::Text(v) => Cell::Text(v),
            Cell::Integer(v) => Cell::Integer(v),
            Cell::Time(v) => Cell::Time(v),
            Cell::Field(v) => Cell::Field(v),
            Cell::Glob(v) => Cell::Glob(v),
            Cell::Regex(v, r) => Cell::Regex(v, r),
            Cell::Op(v) => Cell::Op(v),
            Cell::Command(v) => Cell::Command(v),
            Cell::File(v) => Cell::File(v),
            Cell::Rows(r) => Cell::Rows(r.concrete()),
            Cell::Output(s) => Cell::to_rows(&s),
            Cell::Closure(c) => Cell::Closure(c),
        };
    }

    pub fn file_expand(&self, v: &mut Vec<Box<Path>>) -> Result<(), JobError> {
        match self {
            Cell::Text(s) => v.push(Box::from(Path::new(s.as_ref()))),
            Cell::File(p) => v.push(p.clone()),
            Cell::Glob(pattern) => to_job_error(pattern.glob_files(
                &get_cwd()?, v))?,
            _ => return Err(error("Expected a file name")),
        }
        Ok(())
    }

    pub fn partial_clone(&self) -> Result<Cell, JobError> {
        return match self {
            Cell::Text(v) => Ok(Cell::Text(v.clone())),
            Cell::Integer(v) => Ok(Cell::Integer(v.clone())),
            Cell::Time(v) => Ok(Cell::Time(v.clone())),
            Cell::Field(v) => Ok(Cell::Field(v.clone())),
            Cell::Glob(v) => Ok(Cell::Glob(v.clone())),
            Cell::Regex(v, r) => Ok(Cell::Regex(v.clone(), r.clone())),
            Cell::Op(v) => Ok(Cell::Op(v.clone())),
            Cell::Command(v) => Ok(Cell::Command(v.clone())),
            Cell::File(v) => Ok(Cell::File(v.clone())),
            Cell::Rows(r) => Ok(Cell::Rows(r.partial_clone()?)),
            Cell::Closure(c) => Ok(Cell::Closure(c.clone())),
            Cell::Output(_) => Err(error("Invalid use of stream")),
        };
    }

    pub fn cast(self, new_type: CellDataType) -> Result<Cell, JobError> {
        if self.cell_data_type() == new_type {
            return Ok(self);
        }
        /*
        This function is silly and overly large. Instead of mathcing on every source/destination pair, it should do
        two matches, one to convert any cell to a string, and one to convert a string to any cell. That would shorten
        this monstrosity to a sane size.
        */
        match (self, new_type) {
            (Cell::Text(s), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(s.as_ref())))),
            (Cell::Text(s), CellDataType::Glob) => Ok(Cell::Glob(Glob::new(&s))),
            (Cell::Text(s), CellDataType::Integer) => to_job_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
            (Cell::Text(s), CellDataType::Field) => Ok(Cell::Field(s)),
            (Cell::Text(s), CellDataType::Op) => Ok(Cell::Op(s)),
            (Cell::Text(s), CellDataType::Regex) => to_job_error(Regex::new(s.as_ref()).map(|v| Cell::Regex(s, v))),

            (Cell::File(s), CellDataType::Text) => match s.to_str() {
                Some(s) => Ok(Cell::Text(Box::from(s))),
                None => Err(error("File name is not valid unicode"))
            },
            (Cell::File(s), CellDataType::Glob) => match s.to_str() {
                Some(s) => Ok(Cell::Glob(Glob::new(s))),
                None => Err(error("File name is not valid unicode"))
            },
            (Cell::File(s), CellDataType::Integer) => match s.to_str() {
                Some(s) => to_job_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
                None => Err(error("File name is not valid unicode"))
            },
            (Cell::File(s), CellDataType::Field) => match s.to_str() {
                Some(s) => Ok(Cell::Field(Box::from(s))),
                None => Err(error("File name is not valid unicode"))
            },
            (Cell::File(s), CellDataType::Op) => match s.to_str() {
                Some(s) => Ok(Cell::Op(Box::from(s))),
                None => Err(error("File name is not valid unicode"))
            },
            (Cell::File(s), CellDataType::Regex) => match s.to_str() {
                Some(s) => to_job_error(Regex::new(s.as_ref()).map(|v| Cell::Regex(Box::from(s), v))),
                None => Err(error("File name is not valid unicode"))
            },

            (Cell::Glob(s), CellDataType::Text) => Ok(Cell::Text(s.to_string().clone().into_boxed_str())),
            (Cell::Glob(s), CellDataType::Field) => Ok(Cell::Field(s.to_string().clone().into_boxed_str())),
            (Cell::Glob(s), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(s.to_string().as_str())))),
            (Cell::Glob(s), CellDataType::Integer) => to_job_error(s.to_string().parse::<i128>()).map(|v| Cell::Integer(v)),
            (Cell::Glob(s), CellDataType::Op) => Ok(Cell::op(s.to_string().as_str())),
            (Cell::Glob(g), CellDataType::Regex) => {
                let s = g.to_string().as_str();
                to_job_error(Regex::new(s).map(|v| Cell::Regex(Box::from(s), v)))
            }

            (Cell::Field(s), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(s.as_ref())))),
            (Cell::Field(s), CellDataType::Glob) => Ok(Cell::Glob(Glob::new(&s))),
            (Cell::Field(s), CellDataType::Integer) => to_job_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
            (Cell::Field(s), CellDataType::Text) => Ok(Cell::Text(s)),
            (Cell::Field(s), CellDataType::Op) => Ok(Cell::Op(s)),
            (Cell::Field(s), CellDataType::Regex) => to_job_error(Regex::new(s.as_ref()).map(|v| Cell::Regex(s, v))),

            (Cell::Regex(s, r), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(s.as_ref())))),
            (Cell::Regex(s, r), CellDataType::Glob) => Ok(Cell::Glob(Glob::new(&s))),
            (Cell::Regex(s, r), CellDataType::Integer) => to_job_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
            (Cell::Regex(s, r), CellDataType::Text) => Ok(Cell::Text(s)),
            (Cell::Regex(s, r), CellDataType::Op) => Ok(Cell::Op(s)),
            (Cell::Regex(s, r), CellDataType::Field) => Ok(Cell::File(Box::from(Path::new(s.as_ref())))),

            (Cell::Integer(i), CellDataType::Text) => Ok(Cell::Text(i.to_string().into_boxed_str())),
            (Cell::Integer(i), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(i.to_string().as_str())))),
            (Cell::Integer(i), CellDataType::Glob) => Ok(Cell::Glob(Glob::new(i.to_string().as_str()))),
            (Cell::Integer(i), CellDataType::Field) => Ok(Cell::Field(i.to_string().into_boxed_str())),
            (Cell::Integer(i), CellDataType::Op) => Ok(Cell::Op(i.to_string().into_boxed_str())),
            (Cell::Integer(i), CellDataType::Regex) => {
                let s = i.to_string();
                to_job_error(Regex::new(s.as_str()).map(|v| Cell::Regex(s.into_boxed_str(), v)))
            }
            _ => Err(error("Unimplemented conversion")),
        }
    }
}

impl std::hash::Hash for Cell {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Cell::Text(v) => v.hash(state),
            Cell::Integer(v) => v.hash(state),
            Cell::Time(v) => v.hash(state),
            Cell::Field(v) => v.hash(state),
            Cell::Glob(v) => v.hash(state),
            Cell::Regex(v, _) => v.hash(state),
            Cell::Op(v) => v.hash(state),
            Cell::Command(_) => { panic!("Impossible!") }
            Cell::File(v) => v.hash(state),
            Cell::Rows(v) => v.hash(state),
            Cell::Closure(c) => {}//c.hash(state),
            Cell::Output(o) => {},
        }
    }
}

impl std::cmp::PartialEq for Cell {
    fn eq(&self, other: &Cell) -> bool {
        return match (self, other) {
            (Cell::Text(val1), Cell::Text(val2)) => val1 == val2,
            (Cell::Glob(glb), Cell::Text(val)) => glb.matches(val),
            (Cell::Text(val), Cell::Glob(glb)) => glb.matches(val),
            (Cell::Integer(val1), Cell::Integer(val2)) => val1 == val2,
            (Cell::Time(val1), Cell::Time(val2)) => val1 == val2,
            (Cell::Field(val1), Cell::Field(val2)) => val1 == val2,
            (Cell::Glob(val1), Cell::Glob(val2)) => val1 == val2,
            (Cell::Regex(val1, _), Cell::Regex(val2, _)) => val1 == val2,
            (Cell::Op(val1), Cell::Op(val2)) => val1 == val2,
            (Cell::Command(val1), Cell::Command(val2)) => val1 == val2,
            (Cell::File(val1), Cell::File(val2)) => val1 == val2,
            _ => panic!("Unimplemented"),
        };
    }
}

pub enum Alignment {
    Left,
    Right,
}

impl std::cmp::PartialOrd for Cell {
    fn partial_cmp(&self, other: &Cell) -> Option<Ordering> {
        return match (self, other) {
            (Cell::Text(val1), Cell::Text(val2)) => Some(val1.cmp(val2)),
            (Cell::Field(val1), Cell::Field(val2)) => Some(val1.cmp(val2)),
            (Cell::Glob(val1), Cell::Glob(val2)) => Some(val1.cmp(val2)),
            (Cell::Regex(val1, _), Cell::Regex(val2, _)) => Some(val1.cmp(val2)),
            (Cell::Integer(val1), Cell::Integer(val2)) => Some(val1.cmp(val2)),
            (Cell::Time(val1), Cell::Time(val2)) => Some(val1.cmp(val2)),
            (Cell::Op(val1), Cell::Op(val2)) => Some(val1.cmp(val2)),
            (Cell::File(val1), Cell::File(val2)) => Some(val1.cmp(val2)),
            _ => Option::None,
        };
    }
}

impl std::cmp::Eq for Cell {}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_casts() {
        assert_eq!(Cell::text("112432").cast(CellDataType::Integer).is_err(), false);
        assert_eq!(Cell::text("1d").cast(CellDataType::Integer).is_err(), true);
        assert_eq!(Cell::text("1d").cast(CellDataType::Glob).is_err(), false);
        assert_eq!(Cell::text("1d").cast(CellDataType::File).is_err(), false);
        assert_eq!(Cell::text("1d").cast(CellDataType::Time).is_err(), true);
        assert_eq!(Cell::text("fad").cast(CellDataType::Field).is_err(), false);
        assert_eq!(Cell::text("fad").cast(CellDataType::Op).is_err(), false);
    }
}
