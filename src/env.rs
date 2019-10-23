use crate::namespace::Namespace;
use crate::errors::{error, JobError};
use std::error::Error;
use std::path::Path;
use crate::printer::Printer;
use std::sync::{Arc, Mutex};
use crate::data::Cell;

/**
  This is where we store variables, including functions.

  The data is protected by a mutex, in order to make sure that all threads can read and write
  concurrently.

  The data is protected by an Arc, in order to make sure that it gets deallocated.
*/
#[derive(Clone)]
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

    pub fn declare(&self, name: &str, value: Cell) -> Result<(), JobError> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.declare(name, value);
    }

    pub fn set(&self, name: &str, value: Cell) -> Result<(), JobError> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.set(name, value);
    }

    pub fn remove(&self, name: &str) {
        let mut namespace = self.namespace.lock().unwrap();
        namespace.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<Cell> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.get(name);
    }
}

pub fn get_cwd() -> Result<Box<Path>, JobError> {
    match std::env::current_dir() {
        Ok(d) => Ok(d.into_boxed_path()),
        Err(e) => Err(error(e.description())),
    }
}
