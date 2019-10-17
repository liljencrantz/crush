use crate::namespace::Namespace;
use crate::errors::{error, JobError};
use std::error::Error;
use std::path::Path;

pub struct State {
    pub namespace: Namespace,
}

impl State {
  pub fn new() -> State {
      return State {
          namespace: Namespace::new(),
      };
  }
}

pub fn get_cwd() -> Result<Box<Path>, JobError> {
    match std::env::current_dir() {
        Ok(d) => Ok(d.into_boxed_path()),
        Err(e) => Err(error(e.description())),
    }
}
