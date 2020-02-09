use crate::data::value::Value;
use crate::errors::{JobResult};
use crate::data::{ColumnType};
use std::mem;

#[derive(PartialEq, PartialOrd, Debug, Eq, Hash, Clone)]
pub struct Row {
    pub cells: Vec<Value>,
}

impl Row {

    pub fn materialize(mut self) ->  Row{
        Row {
            cells: self.cells.drain(..).map(|c| c.materialize()).collect(),
        }
    }
}

#[derive(PartialEq, PartialOrd, Debug, Hash, Clone)]
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
