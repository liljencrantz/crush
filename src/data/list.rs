use crate::data::{CellType, Cell};
use crate::errors::{JobError, mandate, JobResult};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;

#[derive(Debug)]
#[derive(Clone)]
pub struct List {
    cell_type: CellType,
    cells: Arc<Mutex<Vec<Cell>>>,
}

impl List {
    pub fn new(cell_type: CellType, cells: Vec<Cell>) -> List { List { cell_type, cells: Arc::from(Mutex::new(cells)) } }

    pub fn to_string(&self) -> String {
        let mut res = "[".to_string();
        let cells = self.cells.lock().unwrap();
        res += &cells.iter().map(|c| c.to_string()).collect::<Vec<String>>().join(" ");
        res += "]";
        res
    }

    pub fn len(&self) -> usize {
        let cells = self.cells.lock().unwrap();
        cells.len()
    }

    pub fn get(&self, idx: usize) -> JobResult<Cell> {
        let cells = self.cells.lock().unwrap();
        mandate(cells.get(idx), "Index out of bounds")?.partial_clone()
    }

    pub fn append(&self, new_cells: &mut Vec<Cell>) {
        let mut cells = self.cells.lock().unwrap();
        cells.append(new_cells);
    }

    pub fn pop(&self) -> Option<Cell> {
        let mut cells = self.cells.lock().unwrap();
        cells.pop()
    }

    pub fn cell_type(&self) -> CellType {
        self.cell_type.clone()
    }

    pub fn partial_clone(&self) -> Result<List, JobError> {
        let cells = self.cells.lock().unwrap();
        Ok(self.clone())
    }
}

impl std::hash::Hash for List {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let cells = self.cells.lock().unwrap();
        for c in cells.iter() {
            c.hash(state);
        }
    }
}

impl std::cmp::PartialEq for List {
    fn eq(&self, other: &List) -> bool {
        false
    }
}

impl std::cmp::PartialOrd for List {
    fn partial_cmp(&self, other: &List) -> Option<Ordering> {
        None
    }
}
