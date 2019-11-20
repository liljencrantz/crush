use crate::namespace::Namespace;
use crate::errors::{error, JobResult};
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use crate::data::{Value, ValueType};
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
        self.declare(&[Box::from(name)], Value::Env(res.clone()))?;
        Ok(res)
    }

    pub fn declare_str(&self, name: &str, value: Value) -> JobResult<()> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.declare(n, value);
    }

    pub fn declare(&self, name: &[Box<str>], value: Value) -> JobResult<()> {
        if name.is_empty() {
            return Err(error("Empty variable name"));
        }
        let mut namespace = self.namespace.lock().unwrap();
        if name.len() == 1 {
            namespace.declare(name[0].as_ref(), value)
        } else {
            match namespace.get(name[0].as_ref()) {
                None => Err(error("Not a namespace")),
                Some(Value::Env(env)) => env.declare(&name[1..name.len()], value),
                _ => Err(error("Unknown namespace")),
            }
        }
    }

    pub fn set_str(&self, name: &str, value: Value) -> JobResult<()> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.set(n, value);
    }

    pub fn set(&self, name: &[Box<str>], value: Value) -> JobResult<()> {
        if name.is_empty() {
            return Err(error("Empty variable name"));
        }
        let mut namespace = self.namespace.lock().unwrap();
        if name.len() == 1 {
            namespace.set(name[0].as_ref(), value)
        } else {
            match namespace.get(name[0].as_ref()) {
                None => Err(error("Not a namespace")),
                Some(Value::Env(env)) => env.set(&name[1..name.len()], value),
                _ => Err(error("Unknown namespace")),
            }
        }
    }

    pub fn remove_str(&self, name: &str) -> Option<Value> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.remove(n);
    }

    pub fn remove(&self, name: &[Box<str>]) -> Option<Value> {
        if name.is_empty() {
            return None;
        }
        let mut namespace = self.namespace.lock().unwrap();
        if name.len() == 1 {
            namespace.remove(name[0].as_ref())
        } else {
            match namespace.get(name[0].as_ref()) {
                None => None,
                Some(Value::Env(env)) => env.remove(&name[1..name.len()]),
                _ => None,
            }
        }
    }

    pub fn get_str(&self, name: &str) -> Option<Value> {
        let n = &name.split('.').map(|e: &str| Box::from(e)).collect::<Vec<Box<str>>>()[..];
        return self.get(n);
    }

    pub fn get(&self, name: &[Box<str>]) -> Option<Value> {
        if name.is_empty() {
            return None;
        }
        let mut namespace = self.namespace.lock().unwrap();
        if name.len() == 1 {
            namespace.get(name[0].as_ref())
        } else {
            match namespace.get(name[0].as_ref()) {
                None => None,
                Some(Value::Env(env)) => env.get(&name[1..name.len()]),
                _ => None,
            }
        }
    }

    pub fn get_location(&self, name: &[Box<str>]) -> Option<(Env, Vec<Box<str>>)> {
        if name.is_empty() {
            return None;
        }
        let mut namespace = self.namespace.lock().unwrap();
        if name.len() == 1 {
            Some((self.clone(), name.to_vec()))
        } else {
            match namespace.get(name[0].as_ref()) {
                None => None,
                Some(Value::Env(env)) => env.get_location(&name[1..name.len()]),
                _ => None,
            }
        }
    }


    pub fn dump(&self, map: &mut HashMap<String, ValueType>) {
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
