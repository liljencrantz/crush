use crate::data::value::Value;
use crate::errors::CrushResult;
use crate::data::ColumnType;
use std::mem;
use crate::replace::Replace;

#[derive(PartialEq, PartialOrd, Debug, Eq, Hash, Clone)]
pub struct Row {
    cells: Vec<Value>,
}

impl Row {
    pub fn new(cells: Vec<Value>) -> Row {
        Row { cells }
    }

    pub fn cells(&self) -> &Vec<Value> {
        &self.cells
    }

    pub fn into_struct(self, types: &Vec<ColumnType>) -> Struct {
        Struct { types: types.clone(), cells: self.cells }
    }

    pub fn into_vec(self) -> Vec<Value> {
        self.cells
    }

    pub fn push(&mut self, value: Value) {
        self.cells.push(value);
    }

    pub fn append(&mut self, values: &mut Vec<Value>) {
        self.cells.append(values);
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn materialize(mut self) -> Row {
        Row {
            cells: self.cells.drain(..).map(|c| c.materialize()).collect(),
        }
    }

    pub fn replace(&mut self, idx: usize, value: Value) -> Value {
        self.cells.replace(idx, value)
    }
}

#[derive(PartialEq, PartialOrd, Debug, Hash, Clone)]
pub struct Struct {
    types: Vec<ColumnType>,
    cells: Vec<Value>,
}

impl Struct {
    pub fn new(mut vec: Vec<(Box<str>, Value)>) -> Struct {
        let types = vec
            .iter()
            .map(|e| ColumnType::named(e.0.as_ref(), e.1.value_type()))
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
        Row { cells: self.cells }
    }

    pub fn into_vec(self) -> Vec<Value> {
        self.cells
    }

    pub fn get(mut self, name: &str) -> Option<Value> {
        for (idx, t) in self.types.iter().enumerate() {
            match &t.name {
                None => {}
                Some(n) => if n.as_ref() == name { return Some(mem::replace(&mut self.cells[idx], Value::Integer(0))); },
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
