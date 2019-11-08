use crate::job::JobDefinition;
use crate::env::Env;
use crate::data::Argument;
use crate::stream::empty_stream;
use crate::errors::{error, JobResult, mandate};
use crate::commands::{CompileContext};
use crate::stream_printer::spawn_print_thread;

#[derive(Clone)]
#[derive(Debug)]
pub struct Closure {
    job_definitions: Vec<JobDefinition>,
    env: Option<Env>,
}

impl Closure {
    pub fn new(job_definitions: Vec<JobDefinition>) -> Closure {
        Closure {
            job_definitions,
            env: None,
        }
    }

    pub fn with_env(&self, env: &Env) -> Closure {
        Closure {
            job_definitions: self.job_definitions.clone(),
            env: Some(env.clone()),
        }
    }

    pub fn spawn_and_execute(&self, context: CompileContext) -> JobResult<()> {
        let job_definitions = self.job_definitions.clone();
        let parent_env = mandate(self.env.clone(), "Closure without env")?;
        let env = parent_env.new_stack_frame();

        Closure::push_arguments_to_env(context.arguments, &env);
        match job_definitions.len() {
            0 => return Err(error("Empty closures not supported")),
            1 => {
                let job = job_definitions[0].spawn_and_execute(&env, &context.printer, context.input, context.output)?;
                job.join(&context.printer);
            }
            _ => {
                {
                    let job_definition = &job_definitions[0];
                    let last_output = spawn_print_thread(&context.printer);
                    let first_job = job_definition.spawn_and_execute(&env, &context.printer, context.input, last_output)?;
                    first_job.join(&context.printer);
                }

                for job_definition in &job_definitions[1..job_definitions.len() - 1] {
                    let last_output = spawn_print_thread(&context.printer);
                    let job = job_definition.spawn_and_execute(&env, &context.printer, empty_stream(), last_output)?;
                    job.join(&context.printer);
                }
                {
                    let job_definition = &job_definitions[job_definitions.len() - 1];
                    let last_job = job_definition.spawn_and_execute(&env, &context.printer, empty_stream(), context.output)?;
                    last_job.join(&context.printer);
                }
            }
        }
        Ok(())
    }

    fn push_arguments_to_env(mut arguments: Vec<Argument>, env: &Env) {
        for arg in arguments.drain(..) {
            if let Some(name) = &arg.name {
                env.declare_str(name.as_ref(), arg.cell);
            }
        }
    }
}
