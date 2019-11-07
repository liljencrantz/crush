use crate::namespace::Namespace;
use crate::errors::{error, JobResult};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::data::{Cell, CellType};
use std::collections::HashMap;

/**
  This is where we store variables, including functions.

  The data is protected by a mutex, in order to make sure that all threads can read and write
  concurrently.

  The data is protected by an Arc, in order to make sure that it gets deallocated.
*/
#[derive(Clone)]
#[derive(Debug)]
pub struct Env {
    namespace: Arc<Mutex<Namespace>>,
}

impl Env {
    pub fn new() -> Env {
        Env {
            namespace: Arc::from(Mutex::new(Namespace::new(None))),
        }
    }

    pub fn new_stack_frame(&self) -> Env {
        Env {
            namespace: Arc::from(Mutex::new(Namespace::new(Some(self.namespace.clone())))),
        }
    }

    pub fn create_namespace(&self, name: &str) -> JobResult<Env> {
        let res = Env {
            namespace: Arc::from(Mutex::new(Namespace::new(None))),
        };
        self.declare(name, Cell::Env(res.clone()))?;
        Ok(res)
    }

    pub fn declare(&self, name: &str, value: Cell) -> JobResult<()> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.declare(name, value);
    }

    pub fn set(&self, name: &str, value: Cell) -> JobResult<()> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.set(name, value);
    }

    pub fn remove(&self, name: &str) {
        let mut namespace = self.namespace.lock().unwrap();
        namespace.remove(name);
    }

    pub fn get(&self, name: &[Box<str>]) -> Option<Cell> {
        if name.is_empty() {
            return None;
        }
        let mut namespace = self.namespace.lock().unwrap();
        if name.len() == 1 {
            namespace.get(name[0].as_ref())
        } else {
            match namespace.get(name[0].as_ref()) {
                None => None,
                Some(Cell::Env(env)) => env.get(&name[1..name.len()]),
                _ => None,
            }
        }
    }

    pub fn dump(&self, map: &mut HashMap<String, CellType>) {
        let namespace = self.namespace.lock().unwrap();
        namespace.dump(map)
    }

    pub fn to_string(&self) -> String {
        let mut map = HashMap::new();
        self.dump(&mut map);
        map.iter().map(|(k, v)| k.clone()).collect::<Vec<String>>().join(", ")
    }
}

pub fn get_cwd() -> JobResult<Box<Path>> {
    match std::env::current_dir() {
        Ok(d) => Ok(d.into_boxed_path()),
        Err(e) => Err(error(e.description())),
    }
}

pub fn get_home() -> JobResult<Box<Path>> {
    match dirs::home_dir() {
        Some(d) => Ok(d.into_boxed_path()),
        None => Err(error("Could not find users home directory")),
    }
}
