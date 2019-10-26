use crate::job::{JobDefinition};
use crate::env::Env;
use std::sync::Arc;
use crate::namespace::Namespace;
use crate::data::{CellDefinition, JobOutput, ColumnType, Argument};
use crate::stream::{InputStream, OutputStream, streams, spawn_print_thread, empty_stream};
use crate::printer::Printer;
use crate::errors::{error, JobError, JobResult, mandate};
use crate::commands::{JobJoinHandle, CompileContext};
use std::thread;
use std::thread::JoinHandle;

#[derive(Clone)]
pub struct ClosureDefinition {
    job_definitions: Vec<JobDefinition>,
    env: Option<Env>,
}

fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

fn handle(h: Result<JoinHandle<JobResult<()>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}

impl ClosureDefinition {
    pub fn new(job_definitions: Vec<JobDefinition>) -> ClosureDefinition {
        ClosureDefinition {
            job_definitions,
            env: None,
        }
    }

    pub fn with_env(&self, env: &Env) -> ClosureDefinition {
        ClosureDefinition {
            job_definitions: self.job_definitions.clone(),
            env: Some(env.clone())
        }
    }

    pub fn spawn_and_execute(&self, context: CompileContext) -> JobResult<JobJoinHandle> {
        let job_definitions = self.job_definitions.clone();
        let parent_env = mandate(self.env.clone(), "Closure without env")?;
        Ok(handle(build("closure".to_string())
            .spawn(move || -> JobResult<()>{
                let env = parent_env.new_stack_frame();

                let mut deps: Vec<JobJoinHandle> = Vec::new();
                let arguments =
                    context.argument_definitions
                        .iter()
                        .map(|a| a.argument(&mut deps, &context.env, &context.printer))
                        .collect::<JobResult<Vec<Argument>>>()?;

                ClosureDefinition::push_arguments_to_env(arguments, &env);
                match job_definitions.len() {
                    0 => return Err(error("Empty closures not supported")),
                    1 => {
                        let mut job = job_definitions[0].spawn_and_execute(&env, &context.printer, context.input, context.output)?;
                        job.join(&context.printer);
                        Ok(())
                    }
                    _ => {
                        {
                            let job_definition = &job_definitions[0];
                            let last_output = spawn_print_thread(&context.printer);
                            let mut first_job = job_definition.spawn_and_execute(&env, &context.printer, context.input, last_output)?;
                            first_job.join(&context.printer);
                        }

                        for job_definition in &job_definitions[1..job_definitions.len() - 1] {
                            let last_output = spawn_print_thread(&context.printer);
                            let mut job = job_definition.spawn_and_execute(&env, &context.printer, empty_stream(), last_output)?;
                            job.join(&context.printer);
                        }
                        {
                            let job_definition = &job_definitions[job_definitions.len() - 1];
                            let mut last_job = job_definition.spawn_and_execute(&env, &context.printer, empty_stream(), context.output)?;
                            last_job.join(&context.printer);
                        }
                        Ok(())
                    }
                }
            })))
    }

    fn push_arguments_to_env(mut arguments: Vec<Argument>, env: &Env) {
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

