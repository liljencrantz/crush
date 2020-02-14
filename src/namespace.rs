use std::collections::HashMap;
use crate::{
    errors::error,
    data::Value,
};
use std::sync::{Mutex, Arc};
use crate::errors::CrushResult;
use crate::data::ValueType;

#[derive(Debug)]
pub struct Namespace {
    parent: Option<Arc<Mutex<Namespace>>>,
    data: HashMap<String, Value>,
}

impl Namespace {
    pub fn new(parent: Option<Arc<Mutex<Namespace>>>) -> Namespace {
        return Namespace {
            parent,
            data: HashMap::new(),
        };
    }

    pub fn declare(&mut self, name: &str, value: Value) -> CrushResult<()> {
        if self.data.contains_key(name) {
            return error(format!("Variable ${{{}}} already exists", name).as_str());
        }
        self.data.insert(name.to_string(), value);
        return Ok(());
    }

    pub fn set(&mut self, name: &str, value: Value) -> CrushResult<()> {
        if !self.data.contains_key(name) {
            match &self.parent {
                Some(p) => {
                    return p.lock().unwrap().set(name, value);
                }
                None => return error(format!("Unknown variable ${{{}}}", name).as_str()),
            }
        }

        if self.data[name].value_type() != value.value_type() {
            return error(format!("Type mismatch when reassigning variable ${{{}}}. Use `unset ${{{}}}` to remove old variable.", name, name).as_str());
        }
        self.data.insert(name.to_string(), value);
        return Ok(());
    }

    pub fn dump(&self, map: &mut HashMap<String, ValueType>) {
        match &self.parent {
            Some(p) => p.lock().unwrap().dump(map),
            None => {}
        }
        for (k, v) in self.data.iter() {
            map.insert(k.clone(), v.value_type());
        }
    }


    pub fn remove(&mut self, name: &str) -> Option<Value> {
        if !self.data.contains_key(name) {
            match &self.parent {
                Some(p) =>
                    p.lock().unwrap().remove(name),
                None => None,
            }
        } else {
            self.data.remove(name)
        }
    }

    pub fn get(&mut self, name: &str) -> Option<Value> {
        match self.data.get(&name.to_string()) {
            Some(v) => Some(v.clone()),
            None => match &self.parent {
                Some(p) => p.lock().unwrap().get(name),
                None => None
            }
        }
    }
}
