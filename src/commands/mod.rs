mod ls_and_find;
mod echo;
mod pwd;
mod cd;
mod filter;
mod sort;
mod set;
mod let_command;

use std::collections::HashMap;
use crate::stream::{InputStream, OutputStream};
use crate::cell::{CellType, Argument, Command, Cell};
use crate::state::State;
use crate::errors::JobError;
use std::{io, thread};
use crate::namespace::Namespace;
use std::thread::JoinHandle;

type Run = fn(
    Vec<CellType>,
    Vec<Argument>,
    InputStream,
    OutputStream) -> Result<(), JobError>;

type Mutate = fn(
    &mut State,
    Vec<CellType>,
    Vec<Argument>) -> Result<(), JobError>;

pub enum Exec {
    Run(Run),
    Mutate(Mutate),
}

pub struct Call {
    name: String,
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    output_type: Vec<CellType>,
    exec: Exec,
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

    pub fn execute(
        self,
        state: &mut State,
        mut input: InputStream,
        mut output: OutputStream) -> Option<JoinHandle<Result<(), JobError>>> {
        return match self.exec {
            Exec::Run(run) => {
                Some(thread::spawn(move || {
                    return run(self.input_type, self.arguments, input, output);
                }))
            }
            Exec::Mutate(mutate) => {
                mutate(state, self.input_type, self.arguments);
                None
            }
        };
    }
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

pub fn add_builtins(namespace: &mut Namespace) {
    namespace.declare("ls", Cell::Command(Command::new(ls_and_find::ls)));
    namespace.declare("find", Cell::Command(Command::new(ls_and_find::find)));
    namespace.declare("echo", Cell::Command(Command::new(echo::echo)));
    namespace.declare("pwd", Cell::Command(Command::new(pwd::pwd)));
    namespace.declare("cd", Cell::Command(Command::new(cd::cd)));
    namespace.declare("filter", Cell::Command(Command::new(filter::filter)));
    namespace.declare("sort", Cell::Command(Command::new(sort::sort)));
    namespace.declare("set", Cell::Command(Command::new(set::set)));
    namespace.declare("let", Cell::Command(Command::new(let_command::let_command)));
}
