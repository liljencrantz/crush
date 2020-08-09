use crate::lang::{execution_context::ExecutionContext, job::JobJoinHandle, command::Command, value::ValueDefinition};
use crate::lang::{argument::ArgumentDefinition, argument::ArgumentVecCompiler, value::Value};
use crate::lang::scope::Scope;
use crate::lang::errors::{error, CrushResult, Kind};
use crate::util::thread::{handle, build};
use std::path::PathBuf;
use crate::lang::execution_context::{JobContext, CompileContext};
use std::ops::Deref;

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
            _ => None
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
    ) -> CrushResult<ExecutionContext> {
        let (arguments, arg_this) =
            local_arguments.compile(&mut job_context.compile_context())?;

        if arg_this.is_some() {
            this = arg_this;
        }

        Ok(job_context.execution_context(arguments, this))
    }

    pub fn can_block(&self, arg: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        let cmd = self.command.compile_internal(context, false);
        match cmd {
            Ok((_, Value::Command(command))) =>
                command.can_block(arg, context) || arg_can_block(&self.arguments, context),
            _ => true,
        }
    }

    pub fn invoke(&self, context: JobContext) -> CrushResult<JobJoinHandle> {
        match self.command.compile_internal(&mut context.compile_context(), false) {
            Ok((this, value)) => {
                invoke_value(this, value, self.arguments.clone(), context)
            }
            Err(err) => {
                if err.kind == Kind::BlockError {
                    let cmd = self.command.clone();
                    let arguments = self.arguments.clone();
                    Ok(handle(build(self.command.to_string().as_str()).spawn(
                        move || {
                            match cmd.clone().compile_unbound(&mut context.compile_context()) {
                                Ok((this, value)) =>
                                    context.printer.handle_error(
                                        invoke_value(this, value, arguments, context.clone())),

                                _ =>
                                    context.printer.handle_error(
                                        try_external_command(cmd, arguments, context.clone())),
                            }
                        })))
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
    context: JobContext) -> CrushResult<JobJoinHandle> {
    match value {
        Value::Command(command) =>
            invoke_command(command, this, local_arguments, context),
        Value::File(f) =>
            if local_arguments.len() == 0 {
                let meta = f.metadata();
                if meta.is_ok() && meta.unwrap().is_dir() {
                    invoke_command(
                        context.env.global_static_cmd(vec!["global", "traversal", "cd"])?,
                        None,
                        vec![ArgumentDefinition::unnamed(ValueDefinition::Value(Value::File(f)))],
                        context)
                } else {
                    invoke_command(
                        context.env.global_static_cmd(vec!["global", "io", "val"])?,
                        None,
                        vec![ArgumentDefinition::unnamed(ValueDefinition::Value(Value::File(f)))],
                        context)
                }
            } else {
                error(format!("Not a command {}", f.to_str().unwrap_or("<invalid filename>")).as_str())
            }
        Value::Type(t) => {
            match t.fields().get("__call_type__") {
                None =>
                    invoke_command(
                        context.env.global_static_cmd(vec!["global", "io", "val"])?,
                        None,
                        vec![ArgumentDefinition::unnamed(ValueDefinition::Value(Value::Type(t)))],
                        context),
                Some(call) =>
                    invoke_command(
                        call.as_ref().copy(),
                        Some(Value::Type(t)),
                        local_arguments,
                        context),
            }
        }
        _ =>
            if local_arguments.len() == 0 {
                invoke_command(
                    context.env.global_static_cmd(vec!["global", "io", "val"])?,
                    None,
                    vec![ArgumentDefinition::unnamed(ValueDefinition::Value(value))],
                    context)
            } else {
                error(format!("Not a command {}", value.to_string()).as_str())
            }
    }
}

fn invoke_command(
    action: Command,
    this: Option<Value>,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext) -> CrushResult<JobJoinHandle> {
    if !action.can_block(&local_arguments, &mut context.compile_context()) && !arg_can_block(&local_arguments, &mut context.compile_context()) {
        let new_context = CommandInvocation::execution_context(
            local_arguments,
            this,
            context.clone())?;
        context.printer.handle_error(action.invoke(new_context));
        Ok(JobJoinHandle::Many(vec![]))
    } else {
        Ok(handle(build(action.name()).spawn(
            move || {
                let res = CommandInvocation::execution_context(
                    local_arguments,
                    this,
                    context.clone());
                if let Ok(ctx) = res {
                    let p = ctx.printer.clone();
                    p.handle_error(action.invoke(ctx));
                } else {
                    context.printer.handle_error(res);
                }
            })))
    }
}

fn try_external_command(
    def: ValueDefinition,
    mut arguments: Vec<ArgumentDefinition>,
    context: JobContext) -> CrushResult<JobJoinHandle> {
    let (cmd, sub) = match def {
        ValueDefinition::Label(str) => (str, None),
        ValueDefinition::GetAttr(parent, sub) =>
            match parent.deref() {
                ValueDefinition::Label(str) => (str.to_string(), Some(sub)),
                _ => return error("Not a command"),
            }
        _ => return error("Not a command"),
    };

    match resolve_external_command(&cmd, &context.env)? {
        None => error(format!("Unknown command name {}", cmd).as_str()),
        Some(path) => {
            arguments.insert(
                0,
                ArgumentDefinition::unnamed(ValueDefinition::Value(Value::File(path))));
            if let Some(subcmd) = sub {
                arguments.insert(
                    1,
                    ArgumentDefinition::unnamed(ValueDefinition::Value(Value::string(subcmd.as_ref()))));
            }
            let call = CommandInvocation {
                command: ValueDefinition::Value(Value::Command(
                    context.env.global_static_cmd(vec!["global", "control", "cmd"])?)),
                arguments,
            };
            call.invoke(context)
        }
    }
}

impl ToString for CommandInvocation {
    fn to_string(&self) -> String {
        self.command.to_string()
    }
}
