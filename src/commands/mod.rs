mod ls;
mod echo;
mod pwd;
mod cd;
mod filter;

use std::collections::HashMap;
use crate::stream::{InputStream, OutputStream};
use crate::result::{CellType, Argument};
use crate::state::State;
use crate::errors::JobError;
use std::{io};
use ls::Ls;
use echo::Echo;
use pwd::Pwd;
use cd::Cd;
use filter::Filter;

pub trait Call {
    fn get_name(&self) -> &String;
    fn get_arguments(&self) -> &Vec<Argument>;
    fn get_input_type(&self) -> &Vec<CellType>;
    fn get_output_type(&self) -> &Vec<CellType>;
    fn run(&mut self, state: &State, input: &mut dyn InputStream, output: &mut dyn OutputStream) -> Result<(), JobError>;
    fn mutate(&mut self, state: &mut State) -> Result<(), JobError>;
}

struct InternalCall {
    name: String,
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    output_type: Vec<CellType>,
    command: Box<dyn InternalCommand>,
}

impl Call for InternalCall {
    fn get_name(&self) -> &String {
        return &self.name;
    }

    fn get_arguments(&self) -> &Vec<Argument> {
        return &self.arguments;
    }

    fn get_input_type(&self) -> &Vec<CellType> {
        return &self.input_type;
    }

    fn get_output_type(&self) -> &Vec<CellType> {
        return &self.output_type;
    }

    fn run(&mut self, state: &State, input: &mut dyn InputStream, output: &mut dyn OutputStream) -> Result<(), JobError> {
        return self.command.run(state, &self.input_type, &self.arguments, input, output);
    }

    fn mutate(&mut self, state: &mut State) -> Result<(), JobError> {
        return self.command.mutate(state, &self.input_type, &self.arguments);
    }
}

pub trait Command {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError>;
}

pub trait InternalCommand {
    fn run(
        &mut self,
        _state: &State,
        _input_type: &Vec<CellType>,
        _arguments: &Vec<Argument>,
        _input: &mut dyn InputStream,
        _output: &mut dyn OutputStream) -> Result<(), JobError> {
        Ok(())
    }

    fn mutate(
        &mut self,
        _state: &mut State,
        _input_type: &Vec<CellType>,
        _arguments: &Vec<Argument>,
        ) -> Result<(), JobError> {
        Ok(())
    }
}

pub struct Namespace {
    commands: HashMap<String, Box<dyn Command>>,
}

fn to_runtime_error(io_result: io::Result<()>) -> Result<(), JobError> {
    return match io_result {
        Ok(_) => {
            Ok(())
        }
        Err(io_err) => {
            Err(JobError{ message: io_err.to_string() })
        }
    }
}


impl Namespace {
    pub fn new() -> Namespace {
        let mut commands: HashMap<String, Box<dyn Command>> = HashMap::new();
        commands.insert(String::from("ls"), Box::new(Ls {}));
        commands.insert(String::from("pwd"), Box::new(Pwd {}));
        commands.insert(String::from("cd"), Box::new(Cd {}));
        commands.insert(String::from("echo"), Box::new(Echo {}));
        commands.insert(String::from("filter"), Box::new(Filter {}));
        let res = Namespace {
            commands,
        };
        return res;
    }

    pub fn call(&self, name: &String, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError> {
        return match self.commands.get(name) {
            Some(cmd) => cmd.call(input_type, arguments),
            None => Result::Err(JobError { message: String::from(format!("Unknown command {}.", name)) }),
        };
    }
}
