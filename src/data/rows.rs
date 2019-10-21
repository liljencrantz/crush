use crate::data::{CellType, Row};
use std::hash::Hasher;
use crate::errors::JobError;

pub struct Rows {
    pub types: Vec<CellType>,
    pub rows: Vec<Row>,
}


impl Rows {
    pub fn concrete(mut self) -> Rows {
        Rows { types: self.types, rows: self.rows.drain(..).map(|c| c.concrete()).collect() }
    }

    pub fn partial_clone(&self) -> Result<Self, JobError> {
        Ok(Rows {
            types: self.types.clone(),
            rows: self.rows.iter().map(|r| r.partial_clone()).collect::<Result<Vec<Row>, JobError>>()?,
        })
    }
}

impl std::hash::Hash for Rows {
    fn hash<H: Hasher>(&self, env: &mut H) {
        for r in &self.rows {
            r.hash(env);
        }
    }
}


