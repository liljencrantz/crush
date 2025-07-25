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
use crate::util::repr::Repr;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
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

        Ok(job_context.command_context(source, arguments, this))
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
        eval(&self.command, &self.source, &self.arguments, context)
    }
}

pub fn eval_non_blocking(
    command: &ValueDefinition,
    source: &Source,
    arguments: &Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    match command.eval(&mut EvalContext::from(&context)) {
        // Try to find the command in this thread. This may fail if the command is found via a subshell, in which case we need to spawn a thread
        Ok((this, value)) => eval_internal(this, value, arguments.clone(), context, source),
        Err(err) => {
            if let ValueDefinition::Identifier(str) = command {
                try_external_command(str, arguments.clone(), context)
            } else {
                return Err(err);
            }
        }
    }
}

pub fn eval(
    command: &ValueDefinition,
    source: &Source,
    arguments: &Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadId>> {
    if command.can_block(&mut EvalContext::from(&context)) {
        eval_non_blocking(command, source, arguments, context)
    } else {
        let command = command.clone();
        let arguments = arguments.clone();
        let my_context = context.clone();
        let thread_source = source.clone();
        Ok(Some(context.spawn(&command.to_string(), move || {
            match eval_non_blocking(&command, &thread_source, &arguments, my_context.clone()) {
                Ok(Some(id)) => my_context
                    .global_state
                    .threads()
                    .join_one(id, &my_context.global_state.printer()),
                Err(e) => my_context.global_state.printer().crush_error(e),
                _ => {}
            }
            Ok(())
        })?))
    }
}

fn eval_internal(
    this: Option<Value>,
    value: Value,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
) -> CrushResult<Option<ThreadId>> {
    match value {
        Value::Command(command) => eval_command(source, command, this, local_arguments, context),
        Value::Type(t) => eval_type(t, local_arguments, context, source),
        Value::Struct(s) => eval_struct(s, local_arguments, context, source),
        v => eval_other(v, local_arguments, context),
    }
}

fn eval_other(
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
        None => eval_command(
            source,
            context
                .scope
                .global_static_cmd(vec!["global", "io", "val"])?,
            None,
            vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                Value::Type(value_type),
                source.clone(),
            ))],
            context,
        ),
        Some(call) => eval_command(
            source,
            call.clone(),
            Some(Value::Type(value_type)),
            local_arguments,
            context,
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
            source,
            call,
            Some(Value::Struct(struct_value)),
            local_arguments,
            context,
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
                eval_command(
                    source,
                    context
                        .scope
                        .global_static_cmd(vec!["global", "io", "val"])?,
                    None,
                    vec![ArgumentDefinition::unnamed(ValueDefinition::Value(
                        Value::Struct(struct_value),
                        source.clone(),
                    ))],
                    context,
                )
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
    source: &Source,
    command: Command,
    this: Option<Value>,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
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
        Ok(Some(context.spawn(&name, move || {
            let res = CommandInvocation::command_context(
                &local_source,
                local_arguments,
                this,
                local_context,
            )?;
            command.eval(res)
        })?))
    }
}

pub fn resolve_external_command(name: &str, env: &Scope) -> CrushResult<Option<PathBuf>> {
    if let Some(Value::List(path)) = env.get("cmd_path")? {
        let path_vec: Vec<_> = path.iter().collect();
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
