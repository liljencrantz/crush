use crate::job::{JobDefinition, Job};
use crate::env::Env;
use std::sync::Arc;
use crate::namespace::Namespace;
use crate::data::{CellDefinition, JobOutput, CellFnurp, Argument};
use crate::stream::{InputStream, OutputStream, streams, spawn_print_thread};
use crate::printer::Printer;
use crate::errors::{error, JobError};
use crate::commands::JobJoinHandle;

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
        let env = parent_env.new_stack_frame();

        self.push_arguments_to_env(arguments, &env);
        match self.job_definitions.len() {
            0 => return Err(error("Empty closures not supported")),
            1 => {
                let mut job = self.job_definitions[0].compile(&env, &printer, initial_input_type, first_input, last_output)?;
                jobs.push(job);
            }
            _ => {
                {
                    let job_definition = &self.job_definitions[0];
                    let (last_output, last_input) = streams();
                    let mut first_job = job_definition.compile(&env, &printer, initial_input_type, first_input, last_output)?;
                    spawn_print_thread(&printer, JobOutput { types: first_job.get_output_type().clone(), stream: last_input });
                    jobs.push(first_job);
                }

                for job_definition in &self.job_definitions[1..self.job_definitions.len() - 1] {
                    let (first_output, first_input) = streams();
                    let (last_output, last_input) = streams();
                    let mut job = job_definition.compile(&env, &printer, &vec![], first_input, last_output)?;
                    spawn_print_thread(&printer, JobOutput { types: job.get_output_type().clone(), stream: last_input });
                    jobs.push(job);
                }

                {
                    let job_definition = &self.job_definitions[self.job_definitions.len() - 1];
                    let (first_output, first_input) = streams();
                    let mut last_job = job_definition.compile(&env, &printer, &vec![], first_input, last_output)?;
                    jobs.push(last_job);
                }
            }
        }
        Ok(Closure {
            jobs,
        })
    }

    fn push_arguments_to_env(&self, mut arguments: Vec<Argument>, env: &Env) {
        for arg in arguments.drain(..) {
            if let Some(name) = &arg.name {
                env.declare(name.as_ref(), arg.cell);
            }
        }
    }

}

impl PartialEq for ClosureDefinition {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

pub struct Closure {
    jobs: Vec<Job>,
}

impl Closure {
    pub fn get_jobs(&self) -> &Vec<Job> {
        &self.jobs
    }

    pub fn execute(mut self) -> JobJoinHandle {
        let mut res: Vec<JobJoinHandle> = Vec::new();
        for mut job in self.jobs {
            res.push(job.execute());
        }
        JobJoinHandle::Many(res)
    }
}

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
