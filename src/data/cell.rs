use crate::glob::Glob;
use std::cmp::Ordering;
use std::hash::Hasher;
use crate::data::{Output, CellDataType, Command, ConcreteRows, ConcreteRow};
use crate::data::row::{Row};
use crate::data::rows::Rows;
use crate::errors::{error, JobError, to_runtime_error};
use std::path::Path;
use regex::Regex;
use chrono::{DateTime, Local};
use crate::state::get_cwd;
use std::ffi::OsStr;
use std::num::ParseIntError;
use std::error::Error;
use crate::job::{JobDefinition, Job};
use crate::closure::Closure;

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
    Closure(Closure),
    JobDefintion(JobDefinition), // During invocation, this will get replaced with an output
    File(Box<Path>),
    Rows(ConcreteRows),
}

impl CellDefinition {
    pub fn cell(self, dependencies: &mut Vec<Job>) -> Result<Cell, JobError> {
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
            CellDefinition::Rows(r) => Cell::Rows(r.rows()),
            CellDefinition::JobDefintion(def) => {
                let mut j = def.job()?;
                let res = Cell::Output(j.take_output().unwrap());
                dependencies.push(j);
                res
            }
            CellDefinition::Closure(c) => Cell::Closure(c),
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
    Command(Command), // This is a cell that contains a crush builtin command
    Closure(Closure),
    Output(Output),
    File(Box<Path>),
    Rows(Rows),
}


#[derive(Clone)]
pub enum ConcreteCell {
    Text(Box<str>),
    Integer(i128),
    Time(DateTime<Local>),
    Field(Box<str>),
    Glob(Glob),
    Regex(Box<str>, Regex),
    Op(Box<str>),
    Command(Command),
    Closure(Closure),
    File(Box<Path>),
    Rows(ConcreteRows),
}

impl ConcreteCell {

    fn to_rows(s: &Output) -> ConcreteCell {
        let mut rows: Vec<ConcreteRow> = Vec::new();
        loop {
            match s.stream.recv() {
                Ok(row) => {
                    rows.push(row.concrete());
                }
                Err(_) => break,
            }
        }
        return ConcreteCell::Rows(ConcreteRows { types: s.types.clone(), rows });
    }

    pub fn to_string(&self) -> String {
        return match self {
            ConcreteCell::Text(val) => val.to_string(),
            ConcreteCell::Integer(val) => val.to_string(),
            ConcreteCell::Time(val) => val.format("%Y-%m-%d %H:%M:%S %z").to_string(),
            ConcreteCell::Field(val) => format!(r"%{}", val),
            ConcreteCell::Glob(val) => format!("*{{{}}}", val.to_string()),
            ConcreteCell::Regex(val, _) => format!("r{{{}}}", val),
            ConcreteCell::Op(val) => val.to_string(),
            ConcreteCell::Command(_) => "Command".to_string(),
            ConcreteCell::File(val) => val.to_str().unwrap_or("<Broken file>").to_string(),
            ConcreteCell::Rows(_) => "<Table>".to_string(),
            ConcreteCell::Closure(_) => "<Closure>".to_string(),
        };
    }

    pub fn alignment(&self) -> Alignment {
        return match self {
            ConcreteCell::Integer(_) => Alignment::Right,
            _ => Alignment::Left,
        };
    }

    pub fn cell(self) -> Cell {
        return match self {
            ConcreteCell::Text(v) => Cell::Text(v),
            ConcreteCell::Integer(v) => Cell::Integer(v),
            ConcreteCell::Time(v) => Cell::Time(v),
            ConcreteCell::Field(v) => Cell::Field(v),
            ConcreteCell::Glob(v) => Cell::Glob(v),
            ConcreteCell::Regex(v, r) => Cell::Regex(v, r),
            ConcreteCell::Op(v) => Cell::Op(v),
            ConcreteCell::Command(v) => Cell::Command(v),
            ConcreteCell::File(v) => Cell::File(v),
            ConcreteCell::Rows(r) => Cell::Rows(r.rows()),
            ConcreteCell::Closure(c) => Cell::Closure(c)
        };
    }

    pub fn to_cell_definition(self) -> CellDefinition {
        return match self {
            ConcreteCell::Text(v) => CellDefinition::Text(v),
            ConcreteCell::Integer(v) => CellDefinition::Integer(v),
            ConcreteCell::Time(v) => CellDefinition::Time(v),
            ConcreteCell::Field(v) => CellDefinition::Field(v),
            ConcreteCell::Glob(v) => CellDefinition::Glob(v),
            ConcreteCell::Regex(v, r) => CellDefinition::Regex(v, r),
            ConcreteCell::Op(v) => CellDefinition::Op(v),
            ConcreteCell::Command(v) => CellDefinition::Command(v),
            ConcreteCell::File(v) => CellDefinition::File(v),
            ConcreteCell::Rows(r) => CellDefinition::Rows(r),
            ConcreteCell::Closure(c) => CellDefinition::Closure(c),
        };
    }

    pub fn cell_data_type(&self) -> CellDataType {
        return match self {
            ConcreteCell::Text(_) => CellDataType::Text,
            ConcreteCell::Integer(_) => CellDataType::Integer,
            ConcreteCell::Time(_) => CellDataType::Time,
            ConcreteCell::Field(_) => CellDataType::Field,
            ConcreteCell::Glob(_) => CellDataType::Glob,
            ConcreteCell::Regex(_, _) => CellDataType::Regex,
            ConcreteCell::Op(_) => CellDataType::Op,
            ConcreteCell::Command(_) => CellDataType::Command,
            ConcreteCell::File(_) => CellDataType::File,
            ConcreteCell::Rows(r) => CellDataType::Rows(r.types.clone()),
            ConcreteCell::Closure(c) => CellDataType::Closure,
        };
    }

}

impl std::hash::Hash for ConcreteCell {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            ConcreteCell::Text(v) => v.hash(state),
            ConcreteCell::Integer(v) => v.hash(state),
            ConcreteCell::Time(v) => v.hash(state),
            ConcreteCell::Field(v) => v.hash(state),
            ConcreteCell::Glob(v) => v.hash(state),
            ConcreteCell::Regex(v, _) => v.hash(state),
            ConcreteCell::Op(v) => v.hash(state),
            ConcreteCell::Command(_) => { panic!("Impossible!") }
            ConcreteCell::File(v) => v.hash(state),
            ConcreteCell::Rows(v) => v.hash(state),
            ConcreteCell::Closure(c) => {}//c.hash(state),
        }
    }
}

impl std::cmp::PartialEq for ConcreteCell {
    fn eq(&self, other: &ConcreteCell) -> bool {
        return match (self, other) {
            (ConcreteCell::Text(val1), ConcreteCell::Text(val2)) => val1 == val2,
            (ConcreteCell::Glob(glb), ConcreteCell::Text(val)) => glb.matches(val),
            (ConcreteCell::Text(val), ConcreteCell::Glob(glb)) => glb.matches(val),
            (ConcreteCell::Integer(val1), ConcreteCell::Integer(val2)) => val1 == val2,
            (ConcreteCell::Time(val1), ConcreteCell::Time(val2)) => val1 == val2,
            (ConcreteCell::Field(val1), ConcreteCell::Field(val2)) => val1 == val2,
            (ConcreteCell::Glob(val1), ConcreteCell::Glob(val2)) => val1 == val2,
            (ConcreteCell::Regex(val1, _), ConcreteCell::Regex(val2, _)) => val1 == val2,
            (ConcreteCell::Op(val1), ConcreteCell::Op(val2)) => val1 == val2,
            (ConcreteCell::Command(val1), ConcreteCell::Command(val2)) => val1 == val2,
            (ConcreteCell::File(val1), ConcreteCell::File(val2)) => val1 == val2,
            _ => panic!("Unimplemented"),
        };
    }
}

pub enum Alignment {
    Left,
    Right,
}

impl std::cmp::PartialOrd for ConcreteCell {
    fn partial_cmp(&self, other: &ConcreteCell) -> Option<Ordering> {
        return match (self, other) {
            (ConcreteCell::Text(val1), ConcreteCell::Text(val2)) => Some(val1.cmp(val2)),
            (ConcreteCell::Field(val1), ConcreteCell::Field(val2)) => Some(val1.cmp(val2)),
            (ConcreteCell::Glob(val1), ConcreteCell::Glob(val2)) => Some(val1.cmp(val2)),
            (ConcreteCell::Regex(val1, _), ConcreteCell::Regex(val2, _)) => Some(val1.cmp(val2)),
            (ConcreteCell::Integer(val1), ConcreteCell::Integer(val2)) => Some(val1.cmp(val2)),
            (ConcreteCell::Time(val1), ConcreteCell::Time(val2)) => Some(val1.cmp(val2)),
            (ConcreteCell::Op(val1), ConcreteCell::Op(val2)) => Some(val1.cmp(val2)),
            (ConcreteCell::File(val1), ConcreteCell::File(val2)) => Some(val1.cmp(val2)),
            _ => Option::None,
        };
    }
}

impl std::cmp::Eq for ConcreteCell {}

impl Cell {
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


    pub fn concrete(self) -> ConcreteCell {
        return match self {
            Cell::Text(v) => ConcreteCell::Text(v),
            Cell::Integer(v) => ConcreteCell::Integer(v),
            Cell::Time(v) => ConcreteCell::Time(v),
            Cell::Field(v) => ConcreteCell::Field(v),
            Cell::Glob(v) => ConcreteCell::Glob(v),
            Cell::Regex(v, r) => ConcreteCell::Regex(v, r),
            Cell::Op(v) => ConcreteCell::Op(v),
            Cell::Command(v) => ConcreteCell::Command(v),
            Cell::File(v) => ConcreteCell::File(v),
            Cell::Rows(r) => ConcreteCell::Rows(r.concrete()),
            Cell::Output(s) => ConcreteCell::to_rows(&s),
            Cell::Closure(c) => ConcreteCell::Closure(c),
        };
    }

    pub fn concrete_copy(&self) -> ConcreteCell {
        return match self {
            Cell::Text(v) => ConcreteCell::Text(v.clone()),
            Cell::Integer(v) => ConcreteCell::Integer(v.clone()),
            Cell::Time(v) => ConcreteCell::Time(v.clone()),
            Cell::Field(v) => ConcreteCell::Field(v.clone()),
            Cell::Glob(v) => ConcreteCell::Glob(v.clone()),
            Cell::Regex(v, r) => ConcreteCell::Regex(v.clone(), r.clone()),
            Cell::Op(v) => ConcreteCell::Op(v.clone()),
            Cell::Command(v) => ConcreteCell::Command(v.clone()),
            Cell::File(v) => ConcreteCell::File(v.clone()),
            Cell::Rows(r) => ConcreteCell::Rows(r.concrete_copy()),
            Cell::Output(o) => ConcreteCell::to_rows(o.clone()),
            Cell::Closure(c) => ConcreteCell::Closure(c.clone()),
        };
    }

    pub fn file_expand(&self, v: &mut Vec<Box<Path>>) -> Result<(), JobError> {
        match self {
            Cell::Text(s) => v.push(Box::from(Path::new(s.as_ref()))),
            Cell::File(p) => v.push(p.clone()),
            Cell::Glob(pattern) => to_runtime_error(pattern.glob_files(
                &get_cwd()?, v))?,
            _ => return Err(error("Expected a file name")),
        }
        Ok(())
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
            (Cell::Text(s), CellDataType::Integer) => to_runtime_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
            (Cell::Text(s), CellDataType::Field) => Ok(Cell::Field(s)),
            (Cell::Text(s), CellDataType::Op) => Ok(Cell::Op(s)),
            (Cell::Text(s), CellDataType::Regex) => to_runtime_error(Regex::new(s.as_ref()).map(|v| Cell::Regex(s, v))),

            (Cell::File(s), CellDataType::Text) => match s.to_str() {
                Some(s) => Ok(Cell::Text(Box::from(s))),
                None => Err(error("File name is not valid unicode"))
            },
            (Cell::File(s), CellDataType::Glob) => match s.to_str() {
                Some(s) => Ok(Cell::Glob(Glob::new(s))),
                None => Err(error("File name is not valid unicode"))
            },
            (Cell::File(s), CellDataType::Integer) => match s.to_str() {
                Some(s) => to_runtime_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
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
                Some(s) => to_runtime_error(Regex::new(s.as_ref()).map(|v| Cell::Regex(Box::from(s), v))),
                None => Err(error("File name is not valid unicode"))
            },

            (Cell::Glob(s), CellDataType::Text) => Ok(Cell::Text(s.to_string().clone().into_boxed_str())),
            (Cell::Glob(s), CellDataType::Field) => Ok(Cell::Field(s.to_string().clone().into_boxed_str())),
            (Cell::Glob(s), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(s.to_string().as_str())))),
            (Cell::Glob(s), CellDataType::Integer) => to_runtime_error(s.to_string().parse::<i128>()).map(|v| Cell::Integer(v)),
            (Cell::Glob(s), CellDataType::Op) => Ok(Cell::op(s.to_string().as_str())),
            (Cell::Glob(g), CellDataType::Regex) => {
                let s = g.to_string().as_str();
                to_runtime_error(Regex::new(s).map(|v| Cell::Regex(Box::from(s), v)))
            },

            (Cell::Field(s), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(s.as_ref())))),
            (Cell::Field(s), CellDataType::Glob) => Ok(Cell::Glob(Glob::new(&s))),
            (Cell::Field(s), CellDataType::Integer) => to_runtime_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
            (Cell::Field(s), CellDataType::Text) => Ok(Cell::Text(s)),
            (Cell::Field(s), CellDataType::Op) => Ok(Cell::Op(s)),
            (Cell::Field(s), CellDataType::Regex) => to_runtime_error(Regex::new(s.as_ref()).map(|v| Cell::Regex(s, v))),

            (Cell::Regex(s, r), CellDataType::File) => Ok(Cell::File(Box::from(Path::new(s.as_ref())))),
            (Cell::Regex(s, r), CellDataType::Glob) => Ok(Cell::Glob(Glob::new(&s))),
            (Cell::Regex(s, r), CellDataType::Integer) => to_runtime_error(s.parse::<i128>()).map(|v| Cell::Integer(v)),
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
                to_runtime_error(Regex::new(s.as_str()).map(|v| Cell::Regex(s.into_boxed_str(), v)))
            },
            _ => Err(error("Unimplemented conversion")),
        }
    }
}


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
