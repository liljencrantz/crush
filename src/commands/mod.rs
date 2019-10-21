mod command_util;

mod ls_and_find;

mod echo;

mod pwd;
mod cd;

mod set;
mod let_command;
mod unset;

mod head;
mod tail;

mod lines;
mod csv;

mod filter;
mod sort;
mod select;
mod enumerate;
mod group;
mod join;
mod count;
mod cat;

mod cast;

use std::{io, thread};
use crate::{
    namespace::Namespace,
    errors::{JobError, error},
    env::Env,
    data::{
        CellType,
        Argument,
        BaseArgument,
        ArgumentDefinition,
        Command,
        Cell,
    },
    stream::{InputStream, OutputStream},
};
use std::thread::JoinHandle;
use std::error::Error;
use crate::printer::Printer;
use crate::data::{CellDefinition, CellDataType, Output};
use crate::job::Job;
use std::sync::{Arc, Mutex};
use crate::closure::Closure;
use crate::stream::{streams, spawn_print_thread};

type CommandInvocation = fn(
    Vec<CellType>,
    Vec<Argument>,
    InputStream,
    OutputStream,
    Env,
    Printer) -> Result<(), JobError>;

#[derive(Clone)]
pub enum Exec {
    Command(CommandInvocation),
    Closure(Closure),
}

pub enum JobResult {
    Many(Vec<JobResult>),
    Async(JoinHandle<Result<(), JobError>>),
    Sync(Result<(), JobError>),
}

impl JobResult {
    pub fn join(self) -> Result<(), JobError> {
        return match self {
            JobResult::Async(a) => match a.join() {
                Ok(r) => r,
                Err(_) => Err(error("Error while wating for command to finish")),
            },
            JobResult::Sync(s) => s,
            JobResult::Many(v) => {
                for j in v {
                    j.join()?;
                }
                Ok(())
            }
        };
    }
}

#[derive(Clone)]
#[derive(PartialEq)]
pub struct CallDefinition {
    name: String,
    arguments: Vec<ArgumentDefinition>,
}

impl CallDefinition {
    pub fn new(name: &str, arguments: Vec<ArgumentDefinition>) -> CallDefinition {
        CallDefinition {
            name: name.to_string(),
            arguments,
        }
    }

    pub fn compile(&self, input_type: Vec<CellType>, dependencies: &mut Vec<Job>, env: &Env, printer: &Printer) -> Result<Call, JobError> {
        let mut args: Vec<Argument> = Vec::new();
        for arg in self.arguments.iter() {
            args.push(arg.argument(dependencies, env, printer)?);
        }
        match &env.get(&self.name) {
            Some(Cell::Command(command)) => {
                let c = command.call;
                return c(input_type, args);
            }
            Some(Cell::Closure(closure)) => {
                let last_job = &closure.get_jobs()[closure.get_jobs().len()];
                return Ok(Call {
                    name: self.name.clone(),
                    input_type,
                    arguments: args,
                    output_type: last_job.,
                    exec: Exec::Closure(closure.clone()),
                });
            }
            _ => {
                return Err(error("Unknown command name"));
            }
        }
    }
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

    fn push_arguments_to_env(&mut self, local_env: &Env) {
        for arg in self.arguments.drain(..) {
            if let Some(name) = &arg.name {
                local_env.declare(name.as_ref(), arg.cell);
            }
        }
    }


    pub fn execute(
        mut self,
        env: &Env,
        printer: &Printer,
        first_input: InputStream,
        last_output: OutputStream) -> JobResult {
        let printer = printer.clone();
        return match self.exec.clone() {
            Exec::Command(run) => {
                let env_copy = env.clone();
                JobResult::Async(thread::Builder::new().name(self.name.clone()).spawn(move || {
                    return run(self.input_type, self.arguments, first_input, last_output, env_copy, printer);
                }).unwrap())
            },

            Exec::Closure(closure) => {
                let mut res: Vec<JobResult> = Vec::new();
                let local_env = closure.get_parent_env().push();
                self.push_arguments_to_env(&local_env);

                match closure.get_jobs().len() {
                    0 => {}
                    1 => {
                        match closure.get_jobs()[0].compile(&local_env, &printer,&self.input_type, first_input, last_output) {
                            Ok(mut job) => {
                                job.exec();
                            }
                            Err(e) => printer.job_error(e),
                        }
                    }
                    _ => {

                        {
                            let job_definition = &closure.get_jobs()[0];
                            let (last_output, last_input) = streams();
                            match job_definition.compile(&local_env, &printer,&self.input_type, first_input, last_output) {
                                Ok(mut job) => {
                                    job.exec();
                                    spawn_print_thread(&printer, Output { types: job.get_output_type().clone(), stream: last_input });
                                }
                                Err(e) => printer.job_error(e),
                            }
                        }

                        for job_definition in &closure.get_jobs()[1..closure.get_jobs().len()-1] {
                            let (first_output, first_input) = streams();
                            let (last_output, last_input) = streams();
                            drop(first_output);

                            match job_definition.compile(&local_env, &printer,&vec![], first_input, last_output) {
                                Ok(mut job) => {
                                    job.exec();
                                    spawn_print_thread(&printer, Output{ types: job.get_output_type().clone(), stream: last_input } );
                                }
                                Err(e) => printer.job_error(e),
                            }
                        }

                        {
                            let job_definition = &closure.get_jobs()[closure.get_jobs().len()-1];
                            let (first_output, first_input) = streams();
                            drop(first_output);

                            match job_definition.compile(&local_env, &printer,&vec![], first_input, last_output) {
                                Ok(mut job) => {
                                    job.exec();
                                }
                                Err(e) => printer.job_error(e),
                            }
                        }
                    }
                }

                JobResult::Many(res)
            }
        };
    }
}

pub fn add_builtins(env: &Env) -> Result<(), JobError> {
    env.declare("ls", Cell::Command(Command::new(ls_and_find::ls)))?;
    env.declare("find", Cell::Command(Command::new(ls_and_find::find)))?;
    env.declare("echo", Cell::Command(Command::new(echo::echo)))?;
    env.declare("pwd", Cell::Command(Command::new(pwd::pwd)))?;
    env.declare("cd", Cell::Command(Command::new(cd::cd)))?;
    env.declare("filter", Cell::Command(Command::new(filter::filter)))?;
    env.declare("sort", Cell::Command(Command::new(sort::sort)))?;
    env.declare("set", Cell::Command(Command::new(set::set)))?;
    env.declare("let", Cell::Command(Command::new(let_command::let_command)))?;
    env.declare("unset", Cell::Command(Command::new(unset::unset)))?;
    env.declare("group", Cell::Command(Command::new(group::group)))?;
    env.declare("join", Cell::Command(Command::new(join::join)))?;
    env.declare("count", Cell::Command(Command::new(count::count)))?;
    env.declare("cat", Cell::Command(Command::new(cat::cat)))?;
    env.declare("select", Cell::Command(Command::new(select::select)))?;
    env.declare("enumerate", Cell::Command(Command::new(enumerate::enumerate)))?;

    env.declare("cast", Cell::Command(Command::new(cast::cast)))?;

    env.declare("head", Cell::Command(Command::new(head::head)))?;
    env.declare("tail", Cell::Command(Command::new(tail::tail)))?;

    env.declare("lines", Cell::Command(Command::new(lines::lines)))?;
    env.declare("csv", Cell::Command(Command::new(csv::csv)))?;

    return Ok(());
}
