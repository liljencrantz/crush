use crate::data::row::Row;
use crate::data::{CellType, ConcreteRow};
use std::hash::Hasher;

pub struct BaseRows<R> {
    pub types: Vec<CellType>,
    pub rows: Vec<R>,
}

pub type ConcreteRows = BaseRows<ConcreteRow>;

impl ConcreteRows {
    pub fn rows(mut self) -> Rows {
        Rows { types: self.types, rows: self.rows.drain(..).map(|r| r.row()).collect()}
    }
}

impl std::hash::Hash for ConcreteRows {
    fn hash<H: Hasher>(&self, env: &mut H) {
        for r in &self.rows {
            r.hash(env);
        }
    }
}

impl Clone for ConcreteRows {
    fn clone(&self) -> Self {
        ConcreteRows {
            types: self.types.clone(),
            rows: self.rows.clone(),
        }
    }
}

pub type Rows = BaseRows<Row>;

impl Rows {
    pub fn concrete(mut self) -> ConcreteRows {
        ConcreteRows { types: self.types, rows: self.rows.drain(..).map(|r| r.concrete()).collect()}
    }

    pub fn concrete_copy(&self) -> ConcreteRows {
        ConcreteRows { types: self.types.clone(), rows: self.rows.iter().map(|r| r.concrete_copy()).collect()}
    }
}
