use crate::namespace::Namespace;
use crate::errors::{error, JobError};
use std::error::Error;

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

pub fn get_cwd() -> Result<String, JobError> {
    match std::env::current_dir() {
        Ok(d) => match d.to_str() {
            Some(s) => Ok(s.to_string()),
            None => Err(error("Current working directory is invalid")),
        },
        Err(e) => Err(error(e.description())),
    }
}
