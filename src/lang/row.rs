use crate::lang::value::Value;
use crate::errors::CrushResult;
use crate::lang::column_type::ColumnType;
use std::mem;
use crate::replace::Replace;
use crate::lang::r#struct::Struct;

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
        Struct::from_vec(self.cells, types.clone())
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

