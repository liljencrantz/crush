use crate::lang::table::ColumnType;
use crate::lang::value::Value;
use std::mem;
use crate::lang::table::Row;

#[derive(PartialEq, PartialOrd, Hash, Clone)]
pub struct Struct {
    types: Vec<ColumnType>,
    cells: Vec<Value>,
}

impl Struct {
    pub fn new(mut vec: Vec<(Box<str>, Value)>) -> Struct {
        let types = vec
            .iter()
            .map(|e| ColumnType::new(e.0.as_ref(), e.1.value_type()))
            .collect();
        let cells = vec
            .drain(..)
            .map(|e| e.1)
            .collect();
        Struct {
            types,
            cells,
        }
    }

    pub fn from_vec(cells: Vec<Value>, types: Vec<ColumnType>) -> Struct {
        Struct {
            cells,
            types,
        }
    }

    pub fn types(&self) -> &Vec<ColumnType> {
        &self.types
    }

    pub fn cells(&self) -> &Vec<Value> {
        &self.cells
    }

    pub fn remove(&mut self, idx: usize) -> Value {
        self.cells.remove(idx)
    }

    pub fn into_row(self) -> Row {
        Row::new(self.cells)
    }

    pub fn into_vec(self) -> Vec<Value> {
        self.cells
    }

    pub fn get(mut self, name: &str) -> Option<Value> {
        for (idx, t) in self.types.iter().enumerate() {
            if t.name.as_ref() == name {
                return Some(mem::replace(&mut self.cells[idx], Value::Integer(0)));
            }
        }
        None
    }

    pub fn idx(&self, idx: usize) -> Option<&Value> {
        self.cells.get(idx)
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
