use crate::data::value::Value;
use crate::errors::{JobResult};
use crate::data::{ColumnType};
use std::mem;

#[derive(PartialEq)]
#[derive(PartialOrd)]
#[derive(Debug, Eq, Hash)]
pub struct Row {
    pub cells: Vec<Value>,
}

impl Row {
    pub fn partial_clone(&self) -> JobResult<Self> {
        Ok(Row {
            cells: self.cells.iter().map(|c| c.partial_clone()).collect::<JobResult<Vec<Value>>>()?,
        })
    }

    pub fn materialize(mut self) ->  Row{
        Row {
            cells: self.cells.drain(..).map(|c| c.materialize()).collect(),
        }
    }
}

#[derive(PartialEq)]
#[derive(PartialOrd)]
#[derive(Debug, Hash)]
pub struct Struct {
    pub types: Vec<ColumnType>,
    pub cells: Vec<Value>,
}

impl Struct {
    pub fn new(mut vec: Vec<(&str, Value)>) -> Struct {
        let types = vec
            .iter()
            .map(|e| ColumnType::named(e.0, e.1.value_type()))
            .collect();
        let cells = vec
            .drain(..)
            .map(|e| e.1)
            .collect();
        Struct {
            types, cells
        }
    }

    pub fn partial_clone(&self) -> JobResult<Self> {
        Ok(Struct {
            types: self.types.clone(),
            cells: self.cells.iter().map(|c| c.partial_clone()).collect::<JobResult<Vec<Value>>>()?,
        })
    }

    pub fn get_types(&self) -> &Vec<ColumnType> {
        &self.types
    }

    pub fn get(mut self, name: &str) -> Option<Value> {
        for (idx, t) in self.types.iter().enumerate() {
            match &t.name {
                None => {},
                Some(n) => if n.as_ref() == name {return Some(mem::replace(&mut self.cells[idx], Value::Integer(0)));},
            }
        }
        None
    }

    pub fn materialize(mut self) -> Struct {
        Struct {
            types: ColumnType::materialize(&self.types),
            cells: self.cells.drain(..).map(|c| c.materialize()).collect(),
        }
    }
}

impl ToString for Struct {
    fn to_string(&self) -> String {
        format!("{{{}}}",
        self.cells
            .iter()
            .zip(self.types.iter())
            .map(|(c, t)| t.format_value(c))
            .collect::<Vec<String>>()
            .join(", "))
    }
}
