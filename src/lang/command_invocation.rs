use crate::lang::errors::{error, CrushResult, CrushErrorType};
use crate::lang::execution_context::{CompileContext, JobContext};
use crate::lang::data::scope::Scope;
use crate::lang::{argument::ArgumentDefinition, argument::ArgumentVecCompiler, value::Value};
use crate::lang::command::Command;
use crate::lang::execution_context::CommandContext;
use crate::lang::value::{ValueDefinition, ValueType};
use std::ops::Deref;
use std::path::PathBuf;
use std::fmt::{Display, Formatter};
use std::thread::ThreadId;
use crate::data::r#struct::Struct;
use crate::lang::ast::Location;

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

    /** Extracts the help message from a closure definition */
    pub fn extract_help_message(&self) -> Option<String> {
        if self.arguments.len() != 0 {
            return None;
        }

        match &self.command {
            ValueDefinition::Value(Value::String(s), _) => Some(s.to_string()),
            _ => None,
        }
    }

    pub fn arguments(&self) -> &[ArgumentDefinition] {
        &self.arguments
    }

    pub fn command(&self) -> &ValueDefinition {
        &self.command
    }

    fn execution_context(
        local_arguments: Vec<ArgumentDefinition>,
        mut this: Option<Value>,
        job_context: JobContext,
    ) -> CrushResult<CommandContext> {
        let (arguments, arg_this) = local_arguments.compile(&mut CompileContext::from(&job_context))?;

        if arg_this.is_some() {
            this = arg_this;
        }

        Ok(job_context.command_context(arguments, this))
    }

    pub fn can_block(&self, arg: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        let cmd = self.command.eval(context, false);
        match cmd {
            Ok((_, Value::Command(command))) => {
                command.can_block(arg, context) || arg_can_block(&self.arguments, context)
            }
            _ => true,
        }
    }

    pub fn eval(&self, context: JobContext) -> CrushResult<Option<ThreadId>> {
        match self.command.eval(&mut CompileContext::from(&context), false)
        {
            Ok((this, value)) => eval_internal(this, value, self.arguments.clone(), context, self.command.location()),
            Err(err) => {
                if err.is(CrushErrorType::BlockError) {
                    let cmd = self.command.clone();
                    let arguments = self.arguments.clone();
                    let t = context.global_state.threads().clone();
                    let location = self.command.location();
                    Ok(Some(t.spawn(
                        &self.command.to_string(),
                        move || {
                            match cmd.clone().eval(&mut CompileContext::from(&context), true) {
                                Ok((this, value)) => context.global_state.printer().handle_error(eval_internal(
                                    this,
                                    value,
                                    arguments,
                                    context.clone(),
                                    location,
                                )),

                                _ => context.global_state.printer().handle_error(try_external_command(
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

fn eval_internal(
    this: Option<Value>,
    value: Value,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    location: Location,
) -> CrushResult<Option<ThreadId>> {
    match value {
        Value::Command(command) => invoke_command(command, this, local_arguments, context),
        Value::File(f) => eval_file(f, local_arguments, context, location),
        Value::Type(t) => eval_type(t, local_arguments, context, location),
        Value::Struct(s) => invoke_struct(s, local_arguments, context, location),
        _ => {
            if local_arguments.len() == 0 {
                invoke_command(
                    context.scope.global_static_cmd(vec!["global", "io", "val"])?,
                    None,
                    vec![ArgumentDefinition::unnamed(ValueDefinition::Value(value, location))],
                    context,
                )
            } else {
                error(&format!("Not a command {}", value))
            }
        }
    }
}

fn eval_file(
    file: PathBuf,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    location: Location,
) -> CrushResult<Option<ThreadId>> {
    if local_arguments.len() == 0 {
        let meta = file.metadata();
        if meta.is_ok() && meta.unwrap().is_dir() {
            invoke_command(
                context
                    .scope
                    .global_static_cmd(vec!["global", "fs", "cd"])?,
                None,
                vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                    Value::File(file),
                    location,
                ))],
                context,
            )
        } else {
            invoke_command(
                context.scope.global_static_cmd(vec!["global", "io", "val"])?,
                None,
                vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                    Value::File(file),
                    location,
                ))],
                context,
            )
        }
    } else {
        error(
            format!(
                "Not a command {}",
                file.to_str().unwrap_or("<invalid filename>")
            )
                .as_str(),
        )
    }
}

fn eval_type(
    value_type: ValueType,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    location: Location,
) -> CrushResult<Option<ThreadId>> {
    match value_type.fields().get("__call__") {
        None => invoke_command(
            context.scope.global_static_cmd(vec!["global", "io", "val"])?,
            None,
            vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                Value::Type(value_type),
                location,
            ))],
            context,
        ),
        Some(call) => invoke_command(
            call.as_ref().copy(),
            Some(Value::Type(value_type)),
            local_arguments,
            context,
        ),
    }
}

fn invoke_struct(
    struct_value: Struct,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    location: Location,
) -> CrushResult<Option<ThreadId>> {
    match struct_value.get("__call__") {
        Some(Value::Command(call)) => {
            invoke_command(call, Some(Value::Struct(struct_value)), local_arguments, context)
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
                    context.scope.global_static_cmd(vec!["global", "io", "val"])?,
                    None,
                    vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                        Value::Struct(struct_value),
                        location,
                    ))],
                    context,
                )
            } else {
                error(
                    format!(
                        "Struct must have a member __call__ to be used as a command {}",
                        struct_value.to_string()
                    )
                        .as_str(),
                )
            }
        }
    }
}

fn invoke_command(
    command: Command,
    this: Option<Value>,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    if !command.can_block(&local_arguments, &mut CompileContext::from(&context))
        && !arg_can_block(&local_arguments, &mut CompileContext::from(&context))
    {
        let new_context =
            CommandInvocation::execution_context(local_arguments, this, context.clone())?;
        context.global_state.printer().handle_error(command.invoke(new_context));
        Ok(None)
    } else {
        let t = context.global_state.threads().clone();
        let name = command.name().to_string();
        Ok(Some(t.spawn(
            &name,
            move || {
                let res = CommandInvocation::execution_context(local_arguments, this, context.clone())?;
                command.invoke(res)
            },
        )?))
    }
}

fn try_external_command(
    def: ValueDefinition,
    mut arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    let def_location = def.location();
    let (cmd, sub) = match def {
        ValueDefinition::Label(str) => (str, None),
        ValueDefinition::GetAttr(parent, sub) => match parent.deref() {
            ValueDefinition::Label(str) => (str.clone(), Some(sub)),
            _ => return error("Not a command"),
        },
        _ => return error("Not a command"),
    };

    match resolve_external_command(&cmd.string, &context.scope)? {
        None => error(format!("Unknown command name {}", cmd).as_str()),
        Some(path) => {
            arguments.insert(
                0,
                ArgumentDefinition::unnamed(ValueDefinition::Value(Value::File(path), cmd.location)),
            );
            if let Some(subcmd) = sub {
                arguments.insert(
                    1,
                    ArgumentDefinition::unnamed(
                        ValueDefinition::Value(
                            Value::string(subcmd.string),
                            subcmd.location,
                        )),
                );
            }
            let call = CommandInvocation {
                command: ValueDefinition::Value(
                    Value::Command(
                        context
                            .scope
                            .global_static_cmd(vec!["global", "control", "cmd"])?,
                    ),
                    def_location,
                ),
                arguments,
            };
            call.eval(context)
        }
    }
}

impl Display for CommandInvocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.command.fmt(f)
    }
}
