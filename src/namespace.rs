use std::collections::HashMap;
use crate::{
    errors::{JobError, error},
    data::{
        CellDefinition,
        Argument,
        Cell,
    },
};
use std::sync::{Mutex, Arc};
use crate::errors::JobResult;

pub struct Namespace {
    parent: Option<Arc<Mutex<Namespace>>>,
    data: HashMap<String, Cell>,
}

impl Namespace {
    pub fn new(parent: Option<Arc<Mutex<Namespace>>>) -> Namespace {
        return Namespace {
            parent,
            data: HashMap::new(),
        };
    }

    pub fn declare(&mut self, name: &str, value: Cell) -> JobResult<()> {
        if self.data.contains_key(name) {
            return Err(error(format!("Variable ${{{}}} already exists", name).as_str()));
        }
        self.data.insert(name.to_string(), value);
        return Ok(());
    }

    pub fn set(&mut self, name: &str, value: Cell) -> JobResult<()> {
        if !self.data.contains_key(name) {
            match &self.parent {
                Some(p) => {
                    return p.lock().unwrap().set(name, value);
                }
                None => return Err(error(format!("Unknown variable ${{{}}}", name).as_str())),
            }
        }

        if self.data[name].cell_type() != value.cell_type() {
            return Err(error(format!("Type mismatch when reassigning variable ${{{}}}. Use `unset ${{{}}}` to remove old variable.", name, name).as_str()));
        }
        self.data.insert(name.to_string(), value);
        return Ok(());
    }

    pub fn remove(&mut self, name: &str) {
        if !self.data.contains_key(name) {
            match &self.parent {
                Some(p) => {
                    return p.lock().unwrap().remove(name);
                }
                None => {}
            }
        }
        self.data.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<Cell> {
        return match self.data.get(&name.to_string()) {
            Some(v) => Some(v.partial_clone().unwrap()),
            None => match &self.parent {
                Some(p) => p.lock().unwrap().get(name),
                None => None
            }
        };
    }
}
