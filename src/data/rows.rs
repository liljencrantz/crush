use crate::data::row::Row;
use crate::data::CellType;
use std::hash::Hasher;

#[derive(Debug)]
pub struct Rows {
    pub types: Vec<CellType>,
    pub rows: Vec<Row>,
}

impl std::hash::Hash for Rows {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for r in &self.rows {
            r.hash(state);
        }
    }
}


impl Clone for Rows {
    fn clone(&self) -> Self {
        Rows {
            types: self.types.clone(),
            rows: self.rows.iter().map(|r| r.concrete()).collect(),
        }
    }
}
