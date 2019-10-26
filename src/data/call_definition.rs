use crate::data::{ArgumentDefinition, ColumnType, Cell, Argument};
use crate::stream::{InputStream, OutputStream, UninitializedInputStream, UninitializedOutputStream};
use crate::printer::Printer;
use crate::env::Env;
use crate::commands::{Call, CompileContext, JobJoinHandle};
use crate::errors::{JobError, error, JobResult};
use std::thread;
use std::thread::JoinHandle;

#[derive(Clone)]
#[derive(PartialEq)]
pub struct CallDefinition {
    name: String,
    arguments: Vec<ArgumentDefinition>,
}

fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

fn handle(h: Result<JoinHandle<JobResult<()>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}

impl CallDefinition {
    pub fn new(name: &str, arguments: Vec<ArgumentDefinition>) -> CallDefinition {
        CallDefinition {
            name: name.to_string(),
            arguments,
        }
    }

    pub fn spawn_and_execute(
        &self,
        env: &Env,
        printer: &Printer,
        input: UninitializedInputStream,
        output: UninitializedOutputStream,
    ) -> JobResult<JobJoinHandle> {
        match &env.get(&self.name) {
            Some(Cell::Command(command)) => {
                let local_printer = printer.clone();
                let local_arguments = self.arguments.clone();
                let local_env = env.clone();
                let c = command.call;
                Ok(handle(build(self.name.clone()).spawn(
                    move || {
                        match c(CompileContext {
                            input,
                            output,
                            argument_definitions: local_arguments,
                            env: local_env,
                            printer: local_printer.clone(),
                        }) {
                            Ok(_) => {},
                            Err(e) => local_printer.job_error(e),
                        }
                        Ok(())
                    })))
            }

            Some(Cell::ClosureDefinition(closure_definition)) => {
                closure_definition.spawn_and_execute(
                    CompileContext {
                        input,
                        output,
                        argument_definitions: self.arguments.clone(),
                        env: env.clone(),
                        printer: printer.clone(),
                    })
            }
            _ => {
                Err(error(format!("Unknown command name {}", &self.name).as_str()))
            }
        }
    }
}
