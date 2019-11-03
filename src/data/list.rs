use crate::data::{CellType, Cell};
use crate::errors::{JobError, mandate, JobResult};
use std::hash::Hasher;
use std::cmp::Ordering;

#[derive(PartialEq)]
#[derive(PartialOrd)]
#[derive(Debug)]
pub struct List {
    cell_type: CellType,
    cells: Vec<Cell>,
}

impl List {
    pub fn new(cell_type: CellType, cells: Vec<Cell>) -> List { List { cell_type, cells } }

    pub fn to_string(&self) -> String {
        let mut res = "[".to_string();
        res += &self.cells.iter().map(|c| c.to_string()).collect::<Vec<String>>().join(" ");
        res += "]";
        res
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn get(&self, idx: usize) -> JobResult<Cell> {
        mandate(self.cells.get(idx), "Index out of bounds")?.partial_clone()
    }

    pub fn cell_type(&self) -> CellType {
        self.cell_type.clone()
    }

    pub fn partial_clone(&self) -> Result<List, JobError> {
        Ok(List {
            cell_type: self.cell_type.clone(),
            cells: self.cells.iter().map(|c| c.partial_clone()).collect::<Result<Vec<Cell>, JobError>>()?
        })
    }
}

impl std::hash::Hash for List {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for c in &self.cells {
            c.hash(state);
        }
    }
}
