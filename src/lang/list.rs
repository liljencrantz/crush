use crate::lang::{value_type::ValueType, value::Value, table::ColumnType, table::Row};
use crate::errors::{CrushError, mandate, CrushResult, error};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use crate::stream::Readable;

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

    pub fn get(&self, idx: usize) -> CrushResult<Value> {
        let cells = self.cells.lock().unwrap();
        Ok(mandate(cells.get(idx), "Index out of bounds")?.clone())
    }

    pub fn set(&self, idx: usize, value: Value) -> CrushResult<()> {
        if self.cell_type != value.value_type() && self.cell_type != ValueType::Any {
            return error("Invalid element type");
        }
        let mut cells = self.cells.lock().unwrap();
        if idx >= cells.len() {
            return error("Index out of range");
        }

        cells[idx] = value;

        Ok(())
    }

    pub fn append(&self, new_cells: &mut Vec<Value>) {
        let mut cells = self.cells.lock().unwrap();
        cells.append(new_cells);
    }

    pub fn dump(&self) -> Vec<Value> {
        let mut res = Vec::new();
        res.append(&mut self.cells.lock().unwrap().clone());
        res
    }

    pub fn pop(&self) -> Option<Value> {
        let mut cells = self.cells.lock().unwrap();
        cells.pop()
    }

    pub fn clear(&self) {
        let mut cells = self.cells.lock().unwrap();
        cells.clear();
    }

    pub fn remove(&self, idx: usize) {
        let mut cells = self.cells.lock().unwrap();
        cells.remove(idx);
    }

    pub fn truncate(&self, idx: usize) {
        let mut cells = self.cells.lock().unwrap();
        cells.truncate(idx);
    }

    pub fn peek(&self) -> Option<Value> {
        let mut cells = self.cells.lock().unwrap();
        cells.get(cells.len()-1).map(|v| v.clone())
    }

    pub fn element_type(&self) -> ValueType {
        self.cell_type.clone()
    }

    pub fn list_type(&self) -> ValueType {
        ValueType::List(Box::from(self.cell_type.clone()))
    }

    pub fn materialize(self) -> List {
        let mut cells = self.cells.lock().unwrap();
        let vec: Vec<Value> = cells.drain(..).map(|c| c.materialize()).collect();
        List {
            cell_type: self.cell_type.materialize(),
            cells: Arc::new(Mutex::from(vec)),
        }
    }

    pub fn copy(&self) -> List {
        let cells = self.cells.lock().unwrap();
        List {
            cell_type: self.cell_type.clone(),
            cells: Arc::from(Mutex::new(cells.clone())),
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

pub struct ListReader {
    list: List,
    idx: usize,
    types: Vec<ColumnType>,
}

impl ListReader {
    pub fn new(list: List,
           name: &str,
    ) -> ListReader {
        ListReader {
            types: vec![ColumnType::named(name, list.element_type())],
            list,
            idx: 0usize,
        }
    }
}

impl Readable for ListReader {
    fn read(&mut self) -> CrushResult<Row> {
        self.idx += 1;
        Ok(Row::new(vec![self.list.get(self.idx - 1)?]))
    }

    fn types(&self) -> &Vec<ColumnType> {
        &self.types
    }
}
