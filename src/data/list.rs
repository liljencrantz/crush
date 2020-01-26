use crate::data::{ValueType, Value};
use crate::errors::{JobError, mandate, JobResult};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;

#[derive(Debug)]
#[derive(Clone)]
pub struct List {
    cell_type: ValueType,
    cells: Arc<Mutex<Vec<Value>>>,
}

impl List {
    pub fn new(cell_type: ValueType, cells: Vec<Value>) -> List { List { cell_type, cells: Arc::from(Mutex::new(cells)) } }

    pub fn len(&self) -> usize {
        let cells = self.cells.lock().unwrap();
        cells.len()
    }

    pub fn get(&self, idx: usize) -> JobResult<Value> {
        let cells = self.cells.lock().unwrap();
        mandate(cells.get(idx), "Index out of bounds")?.partial_clone()
    }

    pub fn append(&self, new_cells: &mut Vec<Value>) {
        let mut cells = self.cells.lock().unwrap();
        cells.append(new_cells);
    }

    pub fn pop(&self) -> Option<Value> {
        let mut cells = self.cells.lock().unwrap();
        cells.pop()
    }

    pub fn element_type(&self) -> ValueType {
        self.cell_type.clone()
    }

    pub fn list_type(&self) -> ValueType {
        ValueType::List(Box::from(self.cell_type.clone()))
    }

    pub fn partial_clone(&self) -> Result<List, JobError> {
        Ok(self.clone())
    }

    pub fn materialize(self) ->  List {
        let mut cells = self.cells.lock().unwrap();
        let vec: Vec<Value> = cells.drain(..).map(|c| c.materialize()).collect();
        List {
            cell_type: self.cell_type.materialize(),
            cells: Arc::new(Mutex::from(vec)),
        }
    }
}

impl ToString for List {
    fn to_string(&self) -> String {
        let mut res = "[".to_string();
        let cells = self.cells.lock().unwrap();
        res += &cells.iter().map(|c| c.to_string()).collect::<Vec<String>>().join(", ");
        res += "]";
        res
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
