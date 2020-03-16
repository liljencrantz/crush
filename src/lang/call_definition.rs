use crate::lang::{command::ExecutionContext, job::JobJoinHandle, command::SimpleCommand, command::Closure, value::ValueDefinition, value::ValueType};
use crate::lang::{argument::ArgumentDefinition, argument::ArgumentVecCompiler, value::Value};
use crate::lang::scope::Scope;
use crate::lang::errors::{error, CrushResult};
use crate::lang::printer::Printer;
use crate::lang::stream::{ValueReceiver, ValueSender, InputStream};
use crate::util::thread::{handle, build};
use std::ops::Deref;
use crate::lang::command::CrushCommand;
use std::path::Path;
use crate::lang::argument::Argument;

#[derive(Clone, Debug)]
pub struct CallDefinition {
    command: ValueDefinition,
    arguments: Vec<ArgumentDefinition>,
}

pub fn format_name(name: &Vec<Box<str>>) -> String {
    name.join(".")
}

fn resolve_external_command(name: &str, env: Scope) -> Option<Box<Path>> {
    if let Value::List(path) = env.get("cmd_path")? {
        let path_vec = path.dump();
        for val in path_vec {
            match val {
                Value::File(el) => {
                    let full = el.join(name);
                    if full.exists() {
                        return Some(full.into_boxed_path());
                    }
                }
                _ => {}
            }
        }
    }
    None
}

fn can_block(local_arguments: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
    for arg in local_arguments {
        if arg.value.can_block(local_arguments, env) {
            return true;
        }
    }
    false
}

impl CallDefinition {
    pub fn new(command: ValueDefinition, arguments: Vec<ArgumentDefinition>) -> CallDefinition {
        CallDefinition { command, arguments }
    }

    pub fn arguments(&self) -> &Vec<ArgumentDefinition> {
        &self.arguments
    }

    /*
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
*/
    fn make_context(
        deps: &mut Vec<JobJoinHandle>,
        local_printer: Printer,
        local_arguments: Vec<ArgumentDefinition>,
        local_env: Scope,
        mut this: Option<Value>,
        input: ValueReceiver,
        output: ValueSender,
    ) -> CrushResult<ExecutionContext> {
        let arguments = local_arguments
            .compile(deps, &local_env, &local_printer)?;

        let arg_this = arguments.iter()
            .filter(|a| a.name.as_ref().map(|e| e.as_ref() == "this").unwrap_or(false))
            .collect::<Vec<&Argument>>();
        if !arg_this.is_empty() {
            this = Some(arg_this.last().unwrap().value.clone());
        }

        Ok(ExecutionContext {
            input,
            output,
            arguments,
            env: local_env,
            this,
            printer: local_printer,
        })
    }

    pub fn can_block(&self, arg: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
        let cmd = self.command.compile_non_blocking(env);
        match cmd {
            Ok((_, Value::Command(command))) =>
                command.can_block(arg, env) || can_block(&self.arguments, env),

            Ok((_, Value::ConditionCommand(command))) =>
                command.can_block(arg, env) || can_block(&self.arguments, env),

            Ok((_, Value::Closure(closure))) =>
                closure.can_block(arg, env) || can_block(&self.arguments, env),

            _ => true,
        }
    }

    fn invoke_command(
        &self,
        action: impl CrushCommand + Send + 'static,
        mut this: Option<Value>,
        local_printer: Printer,
        local_arguments: Vec<ArgumentDefinition>,
        local_env: Scope,
        input: ValueReceiver,
        output: ValueSender,
    ) -> CrushResult<JobJoinHandle> {
        if !action.can_block(&local_arguments, &local_env) && !can_block(&local_arguments, &local_env) {
            let mut deps: Vec<JobJoinHandle> = Vec::new();
            let context = CallDefinition::make_context(
                &mut deps, local_printer,
                local_arguments,
                local_env,
                this,
                input, output)?;
            action.invoke(context)?;
            Ok(JobJoinHandle::Many(deps))
        } else {
            Ok(handle(build(self.command.to_string()).spawn(
                move || {
                    let mut deps: Vec<JobJoinHandle> = Vec::new();
                    let context = CallDefinition::make_context(
                        &mut deps, local_printer.clone(),
                        local_arguments,
                        local_env,
                        this,
                        input, output)?;
                    action.invoke(context)?;
//                    JobJoinHandle::Many(deps).join(&local_printer);
                    Ok(())
                })))
        }
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
        match self.command.compile_non_blocking(env) {
            Ok((this, cmd)) =>
                match cmd {
                    Value::Command(command) =>
                        self.invoke_command(command, this, local_printer, local_arguments, local_env, input, output),
                    Value::ConditionCommand(command) =>
                        self.invoke_command(command, this, local_printer, local_arguments, local_env, input, output),
                    Value::Closure(closure) =>
                        self.invoke_command(closure, this, local_printer, local_arguments, local_env, input, output),
                    Value::File(_) =>
                        if local_arguments.len() == 0 {
                            self.invoke_command(
                                SimpleCommand::new(crate::lib::file::cd, false),
                                None, local_printer,
                                vec![ArgumentDefinition::unnamed(ValueDefinition::Value(cmd))],
                                local_env, input, output)
                        } else {
                            error(format!("Not a command {}", self.command.to_string()).as_str())
                        }
                    _ =>
                        if local_arguments.len() == 0 {
                            self.invoke_command(
                                SimpleCommand::new(crate::lib::io::val, false),
                                None, local_printer,
                                vec![ArgumentDefinition::unnamed(ValueDefinition::Value(cmd))],
                                local_env, input, output)
                        } else {
                            error(format!("Not a command {}", self.command.to_string()).as_str())
                        }
                }
            Err(err) => {
                if let ValueDefinition::Label(p) = &self.command {
                    match resolve_external_command(p, env.clone()) {
                        None => error(format!("Unknown command name {}", self.command.to_string()).as_str()),
                        Some(path) => {
                            local_arguments.insert(
                                0,
                                ArgumentDefinition::unnamed(ValueDefinition::Value(Value::File(path))));
                            self.invoke_command(SimpleCommand::new(crate::lib::cmd, true), None, local_printer, local_arguments, local_env, input, output)
                        }
                    }
                } else {
                    Err(err)
                }
            }
        }
    }
}

impl ToString for CallDefinition {
    fn to_string(&self) -> String {
        self.command.to_string()
    }
}
