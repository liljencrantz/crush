use std::collections::HashMap;
use crate::{
    errors::{JobError, error},
    commands::Call,
    data::{
        CellType,
        Argument,
        Cell
    }
};

pub struct Namespace {
    data: HashMap<String, Cell>,
}

impl Namespace {
    pub fn new() -> Namespace {
        return Namespace {
            data: HashMap::new(),
        };
    }

    pub fn declare(&mut self, name: &str, value: Cell) -> Result<(), JobError> {
        if self.data.contains_key(name) {
            return Err(error(format!("Variable ${{{}}} already exists", name).as_str()));
        }
        self.data.insert(name.to_string(), value);
        return Ok(());
    }

    pub fn set(&mut self, name: &str, value: Cell) -> Result<(), JobError> {
        if !self.data.contains_key(name) {
            return Err(error(format!("Unknown variable ${{{}}}", name).as_str()));
        }
        if self.data[name].cell_data_type() != value.cell_data_type() {
            return Err(error(format!("Type mismatch when reassigning variable ${{{}}}. Use `unset ${{{}}}` to remove old variable.", name, name).as_str()));
        }
        self.data.insert(name.to_string(), value);
        return Ok(());
    }

    pub fn remove(&mut self, name: &str) {
        self.data.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<&Cell> {
        return self.data.get(&name.to_string());
    }

    pub fn call(&self, name: &str, input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
        return match self.data.get(name) {
            Some(Cell::Command(cmd)) => {
                let c = cmd.call;
                return c(input_type, arguments);
            }
            _ => Result::Err(JobError { message: String::from(format!("Unknown command {}.", name)) }),
        };
    }
}
