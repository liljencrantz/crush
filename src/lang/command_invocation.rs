use crate::lang::errors::{error, CrushError, CrushResult};
use crate::lang::execution_context::{CompileContext, JobContext};
use crate::lang::data::scope::Scope;
use crate::lang::{argument::ArgumentDefinition, argument::ArgumentVecCompiler, value::Value};
use crate::lang::{
    command::Command, execution_context::CommandContext,
    value::ValueDefinition,
};
use std::ops::Deref;
use std::path::PathBuf;
use std::fmt::{Display, Formatter};
use std::thread::ThreadId;

#[derive(Clone)]
pub struct CommandInvocation {
    command: ValueDefinition,
    arguments: Vec<ArgumentDefinition>,
}

fn resolve_external_command(name: &str, env: &Scope) -> CrushResult<Option<PathBuf>> {
    if let Some(Value::List(path)) = env.get("cmd_path")? {
        let path_vec = path.dump();
        for val in path_vec {
            match val {
                Value::File(el) => {
                    let full = el.join(name);
                    if full.exists() {
                        return Ok(Some(full));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(None)
}

fn arg_can_block(local_arguments: &Vec<ArgumentDefinition>, context: &mut CompileContext) -> bool {
    for arg in local_arguments {
        if arg.value.can_block(local_arguments, context) {
            return true;
        }
    }
    false
}

impl CommandInvocation {
    pub fn new(command: ValueDefinition, arguments: Vec<ArgumentDefinition>) -> CommandInvocation {
        CommandInvocation { command, arguments }
    }

    pub fn as_string(&self) -> Option<String> {
        if self.arguments.len() != 0 {
            return None;
        }

        match &self.command {
            ValueDefinition::Value(Value::String(s)) => Some(s.to_string()),
            _ => None,
        }
    }

    pub fn arguments(&self) -> &[ArgumentDefinition] {
        &self.arguments
    }

    pub fn command(&self) -> &ValueDefinition {
        &self.command
    }

    /*
        pub fn spawn_stream(
            &self,
            env: &Scope,
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
    fn execution_context(
        local_arguments: Vec<ArgumentDefinition>,
        mut this: Option<Value>,
        job_context: JobContext,
    ) -> CrushResult<CommandContext> {
        let (arguments, arg_this) = local_arguments.compile(&mut job_context.compile_context())?;

        if arg_this.is_some() {
            this = arg_this;
        }

        Ok(job_context.command_context(arguments, this))
    }

    pub fn can_block(&self, arg: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        let cmd = self.command.compile_internal(context, false);
        match cmd {
            Ok((_, Value::Command(command))) => {
                command.can_block(arg, context) || arg_can_block(&self.arguments, context)
            }
            _ => true,
        }
    }

    pub fn invoke(&self, context: JobContext) -> CrushResult<Option<ThreadId>> {
        match self
            .command
            .compile_internal(&mut context.compile_context(), false)
        {
            Ok((this, value)) => invoke_value(this, value, self.arguments.clone(), context),
            Err(err) => {
                if err == CrushError::BlockError {
                    let cmd = self.command.clone();
                    let arguments = self.arguments.clone();
                    let t = context.threads.clone();
                    Ok(Some(t.spawn(
                        &self.command.to_string(),
                        move || {
                            match cmd.clone().compile_unbound(&mut context.compile_context()) {
                                Ok((this, value)) => context.printer.handle_error(invoke_value(
                                    this,
                                    value,
                                    arguments,
                                    context.clone(),
                                )),

                                _ => context.printer.handle_error(try_external_command(
                                    cmd,
                                    arguments,
                                    context.clone(),
                                )),
                            }
                            Ok(())
                        },
                    )?))
                } else {
                    try_external_command(self.command.clone(), self.arguments.clone(), context)
                }
            }
        }
    }
}

fn invoke_value(
    this: Option<Value>,
    value: Value,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    match value {
        Value::Command(command) => invoke_command(command, this, local_arguments, context),
        Value::File(f) => {
            if local_arguments.len() == 0 {
                let meta = f.metadata();
                if meta.is_ok() && meta.unwrap().is_dir() {
                    invoke_command(
                        context
                            .env
                            .global_static_cmd(vec!["global", "traversal", "cd"])?,
                        None,
                        vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                            Value::File(f),
                        ))],
                        context,
                    )
                } else {
                    invoke_command(
                        context.env.global_static_cmd(vec!["global", "io", "val"])?,
                        None,
                        vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                            Value::File(f),
                        ))],
                        context,
                    )
                }
            } else {
                error(
                    format!(
                        "Not a command {}",
                        f.to_str().unwrap_or("<invalid filename>")
                    )
                        .as_str(),
                )
            }
        }
        Value::Type(t) => match t.fields().get("__call__") {
            None => invoke_command(
                context.env.global_static_cmd(vec!["global", "io", "val"])?,
                None,
                vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                    Value::Type(t),
                ))],
                context,
            ),
            Some(call) => invoke_command(
                call.as_ref().copy(),
                Some(Value::Type(t)),
                local_arguments,
                context,
            ),
        },
        Value::Struct(s) => match s.get("__call__") {
            Some(Value::Command(call)) => {
                invoke_command(call, Some(Value::Struct(s)), local_arguments, context)
            }
            Some(v) => error(
                format!(
                    "__call__ should be a command, was of type {}",
                    v.value_type().to_string()
                )
                    .as_str(),
            ),
            _ => {
                if local_arguments.len() == 0 {
                    invoke_command(
                        context.env.global_static_cmd(vec!["global", "io", "val"])?,
                        None,
                        vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                            Value::Struct(s)
                        ))],
                        context,
                    )
                } else {
                    error(
                        format!(
                            "Struct must have a member __call__ to be used as a command {}",
                            s.to_string()
                        )
                            .as_str(),
                    )
                }
            }
        },
        _ => {
            if local_arguments.len() == 0 {
                invoke_command(
                    context.env.global_static_cmd(vec!["global", "io", "val"])?,
                    None,
                    vec![ArgumentDefinition::unnamed(ValueDefinition::Value(value))],
                    context,
                )
            } else {
                error(&format!("Not a command {}", value))
            }
        }
    }
}

fn invoke_command(
    action: Command,
    this: Option<Value>,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    if !action.can_block(&local_arguments, &mut context.compile_context())
        && !arg_can_block(&local_arguments, &mut context.compile_context())
    {
        let new_context =
            CommandInvocation::execution_context(local_arguments, this, context.clone())?;
        context.printer.handle_error(action.invoke(new_context));
        Ok(None)
    } else {
        let t = context.threads.clone();
        let name = action.name().to_string();
        Ok(Some(t.spawn(
            &name,
            move || {
                let res = CommandInvocation::execution_context(local_arguments, this, context.clone())?;
                action.invoke(res)
            },
        )?))
    }
}

fn try_external_command(
    def: ValueDefinition,
    mut arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    let (cmd, sub) = match def {
        ValueDefinition::Label(str) => (str, None),
        ValueDefinition::GetAttr(parent, sub) => match parent.deref() {
            ValueDefinition::Label(str) => (str.to_string(), Some(sub)),
            _ => return error("Not a command"),
        },
        _ => return error("Not a command"),
    };

    match resolve_external_command(&cmd, &context.env)? {
        None => error(format!("Unknown command name {}", cmd).as_str()),
        Some(path) => {
            arguments.insert(
                0,
                ArgumentDefinition::unnamed(ValueDefinition::Value(Value::File(path))),
            );
            if let Some(subcmd) = sub {
                arguments.insert(
                    1,
                    ArgumentDefinition::unnamed(ValueDefinition::Value(Value::string(
                        subcmd.as_ref(),
                    ))),
                );
            }
            let call = CommandInvocation {
                command: ValueDefinition::Value(Value::Command(
                    context
                        .env
                        .global_static_cmd(vec!["global", "control", "cmd"])?,
                )),
                arguments,
            };
            call.invoke(context)
        }
    }
}

impl Display for CommandInvocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.command.fmt(f)
    }
}
