use crate::data::cell::Cell;
use std::cmp::Ordering;
use std::cmp::PartialOrd;
use std::hash::Hasher;
use crate::data::{CellType, ConcreteCell};

pub struct BaseRow<C> {
    pub cells: Vec<C>,
}

pub type Row = BaseRow<Cell>;

impl Row {
    pub fn concrete(mut self) -> ConcreteRow {
        ConcreteRow {cells: self.cells.drain(..).map(|c| c.concrete()).collect()}
    }
}

pub type ConcreteRow = BaseRow<ConcreteCell>;

impl ConcreteRow {
    pub fn row(mut self) -> Row {
        Row {cells: self.cells.drain(..).map(|c| c.cell()).collect()}
    }
}

impl std::hash::Hash for ConcreteRow {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for c in &self.cells {
            c.hash(state);
        }
    }
}

impl Clone for ConcreteRow {
    fn clone(&self) -> Self {
        ConcreteRow {
            cells: self.cells.clone(),
        }
    }
}
