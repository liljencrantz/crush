use crate::data::r#struct::Struct;
use crate::lang::ast::source::Source;
use crate::lang::command::Command;
/// A single command from a larger Job.
///
/// This code is a bit messy, because it is not until we get to this point in the execution of
/// a command that we will figure out if we're running a crush builtin or an external command.
///
/// If the command we are executing is in fact a struct, we call the `__eval__` method on the
/// struct.
///
/// This code path also tries to avoid forking of threads for commands that are known to never
/// block, which again complicates the code a bit.
use crate::lang::errors::{CrushResult, error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::contexts::{EvalContext, JobContext};
use crate::lang::state::scope::Scope;
use crate::lang::value::{ValueDefinition, ValueType};
use crate::lang::{argument::ArgumentDefinition, argument::ArgumentEvaluator, value::Value};
use crate::util::env;
use crate::util::repr::Repr;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use std::thread::ThreadId;

#[derive(Clone)]
pub struct CommandInvocation {
    source: Source,
    command: ValueDefinition,
    arguments: Vec<ArgumentDefinition>,
}

fn arg_can_block(local_arguments: &Vec<ArgumentDefinition>, context: &mut EvalContext) -> bool {
    for arg in local_arguments {
        if arg.value.can_block(context) {
            return true;
        }
    }
    false
}

impl CommandInvocation {
    pub fn new(
        command: ValueDefinition,
        source: Source,
        arguments: Vec<ArgumentDefinition>,
    ) -> CommandInvocation {
        CommandInvocation {
            source,
            command,
            arguments,
        }
    }

    pub fn source(&self) -> &Source {
        &self.source
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

    /**
    Evaluates all the arguments into values, and puts them into a CommandContext,
    ready to be executed by the main command.
     */
    fn command_context(
        source: &Source,
        local_arguments: Vec<ArgumentDefinition>,
        mut this: Option<Value>,
        job_context: JobContext,
    ) -> CrushResult<CommandContext> {
        let (arguments, arg_this) = local_arguments.eval(&mut EvalContext::from(&job_context))?;

        if arg_this.is_some() {
            this = arg_this;
        }

        job_context.command_context(source, arguments, this)
    }

    pub fn can_block(&self, context: &mut EvalContext) -> bool {
        if self.command.can_block(context) {
            return true;
        }
        match self.command.eval(context) {
            Ok((_, Value::Command(command))) => {
                command.might_block(&self.arguments, context)
                    || arg_can_block(&self.arguments, context)
            }
            _ => true,
        }
    }

    pub fn eval(&self, context: JobContext) -> CrushResult<Option<ThreadId>> {
        if !self.command.can_block(&mut EvalContext::from(&context)) {
            eval_value_definition(&self.command, &self.arguments, context, &self.source)
        } else {
            let local_command = self.command.clone();
            let local_arguments = self.arguments.clone();
            let local_context = context.clone();
            let local_source = self.source.clone();
            Ok(Some(context.global_state.threads().spawn(
                &local_command.to_string(),
                &context.handle.current_command_handle(),
                move || {
                    match eval_value_definition(
                        &local_command,
                        &local_arguments,
                        local_context.clone(),
                        &local_source,
                    ) {
                        Ok(Some(id)) => local_context
                            .global_state
                            .threads()
                            .join_one(id, &local_context.global_state.printer()),
                        Err(e) => local_context.global_state.printer().crush_error(e),
                        _ => {}
                    }
                    Ok(())
                },
            )?))
        }
    }
}

pub fn eval_value_definition(
    command: &ValueDefinition,
    arguments: &Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
) -> CrushResult<Option<ThreadId>> {
    match command.eval(&mut EvalContext::from(&context)) {
        // Try to find the command in this thread. This may fail if the command is found via a subshell, in which case we need to spawn a thread
        Ok((this, value)) => {
            let local_arguments = arguments.clone();
            match value {
                Value::Command(command) => {
                    eval_command(command, this, local_arguments, context, source)
                }
                Value::Type(t) => eval_type(t, local_arguments, context, source),
                Value::Struct(s) => eval_struct(s, local_arguments, context, source),
                v => eval_literal_value(v, local_arguments, context),
            }
        }
        Err(err) => {
            if let ValueDefinition::Identifier(str) = command {
                try_external_command(str, arguments.clone(), context)
            } else {
                Err(err)
            }
        }
    }
}

fn eval_literal_value(
    value: Value,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    if local_arguments.len() == 0 {
        context.output.send(value)?;
        Ok(None)
    } else {
        error(&format!("`{}` is not a command.", value))
    }
}

fn eval_type(
    value_type: ValueType,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
) -> CrushResult<Option<ThreadId>> {
    match value_type.fields().get("__call__") {
        None => eval_literal_value(Value::Type(value_type), local_arguments, context),
        Some(call) => eval_command(
            call.clone(),
            Some(Value::Type(value_type)),
            local_arguments,
            context,
            source,
        ),
    }
}

fn eval_struct(
    struct_value: Struct,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
) -> CrushResult<Option<ThreadId>> {
    match struct_value.get("__call__") {
        Some(Value::Command(call)) => eval_command(
            call,
            Some(Value::Struct(struct_value)),
            local_arguments,
            context,
            source,
        ),

        Some(v) => error(
            format!(
                "Member `__call__` must be a command for struct to be callable, was of type {}",
                v.value_type().to_string()
            )
            .as_str(),
        ),

        _ => {
            if local_arguments.len() == 0 {
                eval_literal_value(Value::Struct(struct_value), local_arguments, context)
            } else {
                error(
                    format!(
                        "Struct must have a member `__call__` to be used as a command {}",
                        struct_value.to_string()
                    )
                    .as_str(),
                )
            }
        }
    }
}

fn eval_command(
    command: Command,
    this: Option<Value>,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
) -> CrushResult<Option<ThreadId>> {
    if !command.might_block(&local_arguments, &mut EvalContext::from(&context))
        && !arg_can_block(&local_arguments, &mut EvalContext::from(&context))
    {
        let new_context =
            CommandInvocation::command_context(source, local_arguments, this, context.clone())?;
        context
            .global_state
            .printer()
            .handle_error(command.eval(new_context));
        Ok(None)
    } else {
        let name = command.name().to_string();
        let local_source = source.clone();
        let local_context = context.clone();
        let command_context = CommandInvocation::command_context(
            &local_source,
            local_arguments,
            this,
            local_context,
        )?;
        Ok(Some(context.global_state.threads().spawn(
            &name,
            &command_context.command_handle().clone(),
            move || {
                let printer = command_context.global_state.printer().clone();
                printer.handle_error(command.eval(command_context));
                Ok(())
            },
        )?))
    }
}

pub fn resolve_external_command(name: &str, env: &Scope) -> CrushResult<Option<PathBuf>> {
    let path_str = env::get("PATH")?;
    let path_vec: Vec<_> = path_str.split(':').collect();
    for i in path_vec {
        if let Ok(val) = PathBuf::from_str(i) {
            let full = val.join(name);
            if full.exists() {
                return Ok(Some(full));
            }
        }
    }

    Ok(None)
}

fn try_external_command(
    cmd: &Source,
    mut arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    match resolve_external_command(&cmd.str(), &context.scope)? {
        None => error(format!("Unknown command name `{}`", cmd.str()).as_str()),
        Some(path) => {
            arguments.insert(
                0,
                ArgumentDefinition::unnamed(ValueDefinition::Value(Value::from(path), cmd.clone())),
            );
            let call = CommandInvocation {
                command: ValueDefinition::Value(
                    Value::Command(
                        context
                            .scope
                            .global_static_cmd(vec!["global", "control", "cmd"])?,
                    ),
                    cmd.clone(),
                ),
                arguments,
                source: cmd.clone(),
            };
            call.eval(context)
        }
    }
}

impl Display for CommandInvocation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.command.repr(f)?;
        for a in &self.arguments {
            f.write_str(" ")?;
            a.fmt(f)?;
        }
        Ok(())
    }
}
