use crate::lang::{value::ValueType, value::Value, table::ColumnType, table::Row};
use crate::lang::errors::{mandate, CrushResult, error, argument_error};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use crate::lang::stream::Readable;
use crate::util::identity_arc::Identity;

#[derive(Clone)]
pub struct List {
    cell_type: ValueType,
    cells: Arc<Mutex<Vec<Value>>>,
}

impl Identity for List {
    fn id(&self) -> u64 {
        self.cells.id()
    }
}

macro_rules! dump_to {
    ($name:ident, $destination_type:ident, $value_type:ident, $convert:expr) => {
    pub fn $name(&self, destination: &mut Vec<$destination_type>) -> CrushResult<()> {
        if self.element_type() != ValueType::$value_type {
            error("Wrong list type")
        } else {
            let cells = self.cells.lock().unwrap();
            for el in cells.iter() {
                match el {
                    Value::$value_type(s) => destination.push($convert(s)),
                    _ => return error("Wrong element type"),
                }

            }
            Ok(())
        }
    }

    }

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
        if !self.cell_type.is(&value) {
            return argument_error("Invalid argument type");
        }
        let mut cells = self.cells.lock().unwrap();
        if idx >= cells.len() {
            return error("Index out of range");
        }

        cells[idx] = value;

        Ok(())
    }

    pub fn append(&self, new_cells: &mut Vec<Value>) -> CrushResult<()> {
        let mut cells = self.cells.lock().unwrap();
        for v in new_cells.iter() {
            if !self.cell_type.is(v) {
                return argument_error("Invalid argument type");
            }
        }
        cells.append(new_cells);
        Ok(())
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

    pub fn remove(&self, idx: usize) -> CrushResult<()> {
        let mut cells = self.cells.lock().unwrap();
        if idx >= cells.len() {
            return argument_error("Index out of bounds");
        }
        cells.remove(idx);
        Ok(())
    }

    pub fn insert(&self, idx: usize, value: Value) -> CrushResult<()>{
        let mut cells = self.cells.lock().unwrap();
        if !self.cell_type.is(&value) {
            return argument_error("Invalid argument type");
        }
        if idx > cells.len() {
            return argument_error("Index out of bounds");
        }
        cells.insert(idx, value);
        Ok(())
    }

    pub fn truncate(&self, idx: usize) {
        let mut cells = self.cells.lock().unwrap();
        cells.truncate(idx);
    }

    pub fn peek(&self) -> Option<Value> {
        let cells = self.cells.lock().unwrap();
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

    dump_to!(dump_string, String, String, |e: &String| e.to_string());
    dump_to!(dump_integer, i128, Integer, |v: &i128| *v);
    //dump_to!(dump_bool, bool, Bool, |v: &bool| *v);
    //dump_to!(dump_type, ValueType, Type, |v: &ValueType| v.clone());
    //dump_to!(dump_float, f64, Float, |v: &f64| *v);
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
        let cells = self.cells.lock().unwrap().clone();
        for c in cells.iter() {
            c.hash(state);
        }
    }
}

impl std::cmp::PartialEq for List {
    fn eq(&self, other: &List) -> bool {
        let us = self.cells.lock().unwrap().clone();
        let them = other.cells.lock().unwrap().clone();
        if us.len() != them.len() {
            return false;
        }
        for (v1, v2) in us.iter().zip(them.iter()) {
            if !v1.eq(v2) {
                return false;
            }
        }
        true
    }
}

impl std::cmp::PartialOrd for List {
    fn partial_cmp(&self, other: &List) -> Option<Ordering> {
        let us = self.cells.lock().unwrap().clone();
        let them = other.cells.lock().unwrap().clone();
        for (v1, v2) in us.iter().zip(them.iter()) {
            let d = v1.partial_cmp(v2);
            match d.clone() {
                Some(Ordering::Equal) => {},
                _ => return d,
            }
        }
        if us.len() != them.len() {
            return us.len().partial_cmp(&them.len());
        }
        Some(Ordering::Equal)
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
            types: vec![ColumnType::new(name, list.element_type())],
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

    fn types(&self) -> &[ColumnType] {
        &self.types
    }
}
