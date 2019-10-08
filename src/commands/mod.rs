mod ls;
mod echo;
mod pwd;
mod cd;
mod filter;
mod sort;

use std::collections::HashMap;
use crate::stream::{InputStream, OutputStream};
use crate::cell::{CellType, Argument};
use crate::state::State;
use crate::errors::JobError;
use std::io;

#[derive(Clone)]
pub struct Call {
    name: String,
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    output_type: Vec<CellType>,
    run: Option<fn(
        &Vec<CellType>,
        &Vec<Argument>,
        &mut InputStream,
        &mut OutputStream) -> Result<(), JobError>>,
    mutate: Option<fn(
        &mut State,
        &Vec<CellType>,
        &Vec<Argument>) -> Result<(), JobError>>,
}

impl Call {
    pub fn get_name(&self) -> &String {
        return &self.name;
    }

    pub fn get_arguments(&self) -> &Vec<Argument> {
        return &self.arguments;
    }

    pub fn get_input_type(&self) -> &Vec<CellType> {
        return &self.input_type;
    }

    pub fn get_output_type(&self) -> &Vec<CellType> {
        return &self.output_type;
    }

    pub fn run(&mut self, input: &mut InputStream, output: &mut OutputStream) -> Result<(), JobError> {
        match self.run {
            Some(r) => r(&self.input_type, &self.arguments, input, output),
            None => Ok(()),
        }
    }

    pub fn mutate(&mut self, state: &mut State) -> Result<(), JobError> {
        match self.mutate {
            Some(m) => m(state, &self.input_type, &self.arguments),
            None => Ok(()),
        }
    }
}

pub struct Namespace {
    commands: HashMap<String, fn(&Vec<CellType>, &Vec<Argument>) -> Result<Call, JobError>>,
}

fn to_runtime_error(io_result: io::Result<()>) -> Result<(), JobError> {
    return match io_result {
        Ok(_) => {
            Ok(())
        }
        Err(io_err) => {
            Err(JobError { message: io_err.to_string() })
        }
    };
}


impl Namespace {
    pub fn new() -> Namespace {
        let mut commands: HashMap<String, fn(&Vec<CellType>, &Vec<Argument>) -> Result<Call, JobError>> = HashMap::new();
        commands.insert(String::from("ls"), ls::ls);
        commands.insert(String::from("echo"), echo::echo);
        commands.insert(String::from("pwd"), pwd::pwd);
        commands.insert(String::from("cd"), cd::cd);
        commands.insert(String::from("filter"), filter::filter);
        commands.insert(String::from("sort"), sort::sort);
        let res = Namespace {
            commands,
        };
        return res;
    }

    pub fn call(&self, name: &String, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Call, JobError> {
        return match self.commands.get(name) {
            Some(cmd) => cmd(input_type, arguments),
            None => Result::Err(JobError { message: String::from(format!("Unknown command {}.", name)) }),
        };
    }
}
