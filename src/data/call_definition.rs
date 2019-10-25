use crate::data::{ArgumentDefinition, ColumnType, Cell, Argument};
use crate::stream::{InputStream, OutputStream};
use crate::printer::Printer;
use crate::env::Env;
use crate::commands::{Call, Exec, CompileContext};
use crate::errors::{JobError, error};
use crate::job::Job;

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

    pub fn compile(
        &self,
        env: &Env,
        printer: &Printer,
        input_type: Vec<ColumnType>,
        input: InputStream,
        output: OutputStream,
        dependencies: &mut Vec<Job>,
    ) -> Result<Call, JobError> {
        let mut args: Vec<Argument> = Vec::new();
        for arg in self.arguments.iter() {
            args.push(arg.argument(dependencies, env, printer)?);
        }
        match &env.get(&self.name) {
            Some(Cell::Command(command)) => {
                let c = command.call;
                let (exec, output_type) = c(CompileContext {
                    input_type,
                    input,
                    output,
                    arguments: args,
                    env: env.clone(),
                    printer: printer.clone()
                })?;
                return Ok(Call::new(
                    self.name.clone(),
                    output_type,
                    exec,
                    printer.clone(),
                    env.clone(),
                ));
            }

            Some(Cell::ClosureDefinition(closure_definition)) => {
                let mut jobs: Vec<Job> = Vec::new();

                let closure = closure_definition.compile(env, printer, &input_type,
                                                         input, output,
                                                         args)?;
                let last_job = &closure.get_jobs()[closure.get_jobs().len() - 1];

                return Ok(Call::new(
                    self.name.clone(),
                    last_job.get_output_type().clone(),
                    Exec::Closure(closure),
                    printer.clone(),
                    env.clone(),
                ));
            }
            _ => {
                return Err(error(format!("Unknown command name {}", &self.name).as_str()));
            }
        }
    }
}
