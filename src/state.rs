use crate::namespace::Namespace;
use crate::errors::{error, JobError};
use std::error::Error;
use std::path::Path;
use crate::printer::Printer;
use std::sync::{Arc, Mutex};
use crate::data::ConcreteCell;

#[derive(Clone)]
pub struct State {
    namespace: Arc<Mutex<Namespace>>,
}

impl State {
  pub fn new() -> State {
      return State {
          namespace: Arc::from(Mutex::new(Namespace::new())),
      };
  }

    pub fn declare(&self, name: &str, value: ConcreteCell) -> Result<(), JobError> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.declare(name, value);
    }

    pub fn set(&self, name: &str, value: ConcreteCell) -> Result<(), JobError> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.set(name, value);
    }

    pub fn remove(&self, name: &str) {
        let mut namespace = self.namespace.lock().unwrap();
        namespace.remove(name);
    }

    pub fn get(&self, name: &str) -> Option<ConcreteCell> {
        let mut namespace = self.namespace.lock().unwrap();
        return namespace.get(name).cloned();
    }
}

pub fn get_cwd() -> Result<Box<Path>, JobError> {
    match std::env::current_dir() {
        Ok(d) => Ok(d.into_boxed_path()),
        Err(e) => Err(error(e.description())),
    }
}
