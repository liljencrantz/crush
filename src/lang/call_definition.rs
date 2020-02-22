use crate::lang::{ExecutionContext, JobJoinHandle};
use crate::lang::{ArgumentDefinition, ArgumentVecCompiler, Value};
use crate::scope::Scope;
use crate::errors::{error, CrushResult};
use crate::printer::Printer;
use crate::stream::{ValueReceiver, ValueSender, InputStream};
use crate::thread_util::{handle, build};

#[derive(Clone)]
#[derive(Debug)]
pub struct CallDefinition {
    name: Vec<Box<str>>,
    arguments: Vec<ArgumentDefinition>,
}

pub fn format_name(name: &Vec<Box<str>>) -> String {
    name.join(".")
}


impl CallDefinition {
    pub fn new(name: Vec<Box<str>>, arguments: Vec<ArgumentDefinition>) -> CallDefinition {
        CallDefinition { name, arguments }
    }

    pub fn spawn_stream(
        &self,
        env: &Scope,
        printer: &Printer,
        mut argument_stream: InputStream,
        output: ValueSender,
    ) -> CrushResult<JobJoinHandle> {
        let cmd = env.get(&self.name);
        match cmd {
            Some(Value::Command(command)) => {
                let c = command.call;
                Ok(handle(build(format_name(&self.name)).spawn(
                    move || {

                        loop {
                            match argument_stream.recv() {
                                Ok(mut row) => {

                                }
                                Err(_) => break,
                            }
                        }

                        Ok(())
                    })))
            }
            _ => {
                error("Can't stream call")
            }
        }


    }

    pub fn spawn_and_execute(
        &self,
        env: &Scope,
        printer: &Printer,
        input: ValueReceiver,
        output: ValueSender,
    ) -> CrushResult<JobJoinHandle> {
        let local_printer = printer.clone();
        let local_arguments = self.arguments.clone();
        let local_env = env.clone();
        let cmd = env.get(&self.name);
        match cmd {
            Some(Value::Command(command)) => {
                let c = command.call;
                Ok(handle(build(format_name(&self.name)).spawn(
                    move || {
                        let mut deps: Vec<JobJoinHandle> = Vec::new();
                        let arguments = local_arguments.compile(&mut deps, &local_env, &local_printer)?;
                        let res = c(ExecutionContext {
                            input,
                            output,
                            arguments,
                            env: local_env,
                            printer: local_printer.clone(),
                        });
                        if !deps.is_empty() {
                            local_printer.join(JobJoinHandle::Many(deps));
                        }
                        res
                    })))
            }

            Some(Value::Closure(closure_definition)) => {
                Ok(handle(build(format_name(&self.name)).spawn(
                    move || {
                        let mut deps: Vec<JobJoinHandle> = Vec::new();
                        let arguments = local_arguments.compile(&mut deps, &local_env, &local_printer)?;

                        closure_definition.spawn_and_execute(
                            ExecutionContext {
                                input,
                                output,
                                arguments,
                                env: local_env.clone(),
                                printer: local_printer.clone(),
                            })?;
                        JobJoinHandle::Many(deps).join(&local_printer);
                        Ok(())
                    })))
            }
            _ => {
                error(format!("Unknown command name {}", format_name(&self.name)).as_str())
            }
        }
    }
}

impl ToString for CallDefinition {
    fn to_string(&self) -> String {
        self.name.last().unwrap().to_string()
    }
}
