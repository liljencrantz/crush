use crate::lang::{ExecutionContext, JobJoinHandle, SimpleCommand, Closure, ValueDefinition, ValueType};
use crate::lang::{ArgumentDefinition, ArgumentVecCompiler, Value};
use crate::scope::Scope;
use crate::errors::{error, CrushResult};
use crate::printer::Printer;
use crate::stream::{ValueReceiver, ValueSender, InputStream};
use crate::thread_util::{handle, build};
use std::ops::Deref;
use crate::lang::command::CrushCommand;
use std::path::Path;

#[derive(Clone)]
#[derive(Debug)]
pub struct CallDefinition {
    name: Vec<Box<str>>,
    arguments: Vec<ArgumentDefinition>,
}

pub fn format_name(name: &Vec<Box<str>>) -> String {
    name.join(".")
}

fn resolve_external_command(name: &str, env: Scope) -> Option<Box<Path>> {
    if let Value::List(path) = env.get_str("cmd_path")? {
        let path_vec = path.dump();
        for val in path_vec {
            match val {
                Value::File(el) => {
                    let full = el.join(name);
                    if full.exists() {
                        return Some(full.into_boxed_path())
                    }
                }
                _ => {}
            }
        }
    }
    None
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
                                Ok(mut row) => {}
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

    fn make_context(
        deps: &mut Vec<JobJoinHandle>,
        local_printer: Printer,
        local_arguments: Vec<ArgumentDefinition>,
        local_env: Scope,
        input: ValueReceiver,
        output: ValueSender,
    ) -> CrushResult<ExecutionContext> {
        let arguments = local_arguments
            .compile(deps, &local_env, &local_printer)?;
        Ok(ExecutionContext {
            input,
            output,
            arguments,
            env: local_env,
            printer: local_printer,
        })
    }

    fn invoke_in_thread(
        &self,
        action: impl CrushCommand + Send + 'static,
        local_printer: Printer,
        local_arguments: Vec<ArgumentDefinition>,
        local_env: Scope,
        input: ValueReceiver,
        output: ValueSender,
    ) -> CrushResult<JobJoinHandle> {
        Ok(handle(build(format_name(&self.name)).spawn(
            move || {
                let mut deps: Vec<JobJoinHandle> = Vec::new();
                let context = CallDefinition::make_context(
                    &mut deps, local_printer.clone(),
                    local_arguments,
                    local_env,
                    input, output)?;
                action.invoke(context)?;
                JobJoinHandle::Many(deps).join(&local_printer);
                Ok(())
            })))
    }

    pub fn invoke(
        &self,
        env: &Scope,
        printer: &Printer,
        input: ValueReceiver,
        output: ValueSender,
    ) -> CrushResult<JobJoinHandle> {
        let local_printer = printer.clone();
        let mut local_arguments = self.arguments.clone();
        let local_env = env.clone();
        let cmd = env.get(&self.name);

        match cmd {
            Some(Value::Command(command)) => {
                self.invoke_in_thread(command, local_printer, local_arguments, local_env, input, output)
            }

            Some(Value::Closure(closure)) => {
                self.invoke_in_thread(closure, local_printer, local_arguments, local_env, input, output)
            }
            None =>
                if self.name.len() == 1 {
                    match resolve_external_command(self.name[0].deref(), env.clone()) {
                        None => error(format!("Unknown command name {}", format_name(&self.name)).as_str()),
                        Some(path) => {
                            local_arguments.insert(0,
                                                   ArgumentDefinition::unnamed(ValueDefinition::Value(Value::File(path))));
                            self.invoke_in_thread(SimpleCommand::new(crate::lib::cmd), local_printer, local_arguments, local_env, input, output)
                        }
                    }
                } else {
                    error(format!("Unknown command name {}", format_name(&self.name)).as_str())
                },
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
