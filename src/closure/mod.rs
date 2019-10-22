use crate::job::{JobDefinition, Job};
use crate::env::Env;
use std::sync::Arc;
use crate::namespace::Namespace;
use crate::data::{CellDefinition, JobOutput, CellFnurp, Argument};
use crate::stream::{InputStream, OutputStream, streams, spawn_print_thread};
use crate::printer::Printer;
use crate::errors::{error, JobError};
use crate::commands::JobResult;

#[derive(Clone)]
pub struct ClosureDefinition {
    job_definitions: Vec<JobDefinition>,
}

impl ClosureDefinition {
    pub fn new(job_definitions: Vec<JobDefinition>) -> ClosureDefinition {
        ClosureDefinition {
            job_definitions,
        }
    }

    pub fn compile(
        &self,
        parent_env: &Env,
        printer: &Printer,
        initial_input_type: &Vec<CellFnurp>,
        first_input: InputStream,
        last_output: OutputStream,
    arguments: Vec<Argument>) -> Result<Closure, JobError> {
        let mut jobs: Vec<Job> = Vec::new();

        match self.job_definitions.len() {
            0 => return Err(error("Empty closures not supported")),
            1 => {
                match self.job_definitions[0].compile(parent_env, &printer, initial_input_type, first_input, last_output) {
                    Ok(mut job) => {
                        jobs.push(job);
                    }
                    Err(e) => printer.job_error(e),
                }
            }
            _ => {
                {
                    let job_definition = &self.job_definitions[0];
                    let (last_output, last_input) = streams();
                    match job_definition.compile(parent_env, &printer, initial_input_type, first_input, last_output) {
                        Ok(mut job) => {
                            spawn_print_thread(&printer, JobOutput { types: job.get_output_type().clone(), stream: last_input });
                            jobs.push(job);
                        }
                        Err(e) => printer.job_error(e),
                    }
                }

                for job_definition in &self.job_definitions[1..self.job_definitions.len() - 1] {
                    let (first_output, first_input) = streams();
                    let (last_output, last_input) = streams();
                    drop(first_output);

                    match job_definition.compile(parent_env, &printer, &vec![], first_input, last_output) {
                        Ok(mut job) => {
                            spawn_print_thread(&printer, JobOutput { types: job.get_output_type().clone(), stream: last_input });
                            jobs.push(job);
                        }
                        Err(e) => printer.job_error(e),
                    }
                }

                {
                    let job_definition = &self.job_definitions[self.job_definitions.len() - 1];
                    let (first_output, first_input) = streams();
                    drop(first_output);

                    match job_definition.compile(parent_env, &printer, &vec![], first_input, last_output) {
                        Ok(mut job) => {
                            jobs.push(job);
                        }
                        Err(e) => printer.job_error(e),
                    }
                }
            }
        }

        Ok(Closure {
            jobs,
            parent_env: parent_env.clone(),
            arguments,
        })
    }
}

impl PartialEq for ClosureDefinition {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

//#[derive(Clone)]
pub struct Closure {
    jobs: Vec<Job>,
    parent_env: Env,
    arguments: Vec<Argument>,
}

impl Closure {
    pub fn get_jobs(&self) -> &Vec<Job> {
        &self.jobs
    }
    pub fn get_parent_env(&self) -> &Env {
        &self.parent_env
    }
/*
    fn push_arguments_to_env(&mut self, local_env: &Env) {
        for arg in self.arguments.drain(..) {
            if let Some(name) = &arg.name {
                local_env.declare(name.as_ref(), arg.cell);
            }
        }
    }
*/
    pub fn execute(self) -> JobResult {
        let mut res: Vec<JobResult> = Vec::new();
        for mut job in self.jobs {
            res.push(job.execute());
        }
        JobResult::Many(res)
    }

}

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
