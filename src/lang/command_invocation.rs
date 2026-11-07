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
use crate::lang::errors::{argument_error, command_error, error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::contexts::{EvalContext, JobContext};
use crate::lang::value::{ValueDefinition, ValueType};
use crate::lang::{argument::ArgumentDefinition, argument::ArgumentEvaluator, value::Value};
use crate::util::directory_lister::{DirectoryLister, directory_lister};
use crate::util::env;
use crate::util::repr::Repr;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::str::FromStr;
use crate::lang::threads::ThreadHandle;

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

    pub fn eval(&self, context: JobContext) -> CrushResult<Option<ThreadHandle>> {
        // Captured once, here, while `self` (the whole invocation -- command position
        // plus every argument, exactly as written) is still available: by the time
        // dispatch reaches `eval_command` below, only the already-*resolved* `Command`
        // value is left, which can't reconstruct the original invocation text on its
        // own (its own `Display` is just its bare name). Threaded down as plain data
        // instead, for `JobHandle::set_command_info` (see `commands`, the builtin this
        // exists for) to record once the command's own `CommandId` is minted.
        let definition = self.to_string();
        if !self.command.can_block(&mut EvalContext::from(&context)) {
            eval_command_definition(&self.command, &self.arguments, context, &self.source, definition)
        } else {
            let local_command = self.command.clone();
            let local_arguments = self.arguments.clone();
            let local_context = context.clone();
            let local_source = self.source.clone();
            Ok(Some(context.global_state.threads().spawn(
                &local_command.to_string(),
                &context.handle.current_command_handle(),
                move || match eval_command_definition(
                    &local_command,
                    &local_arguments,
                    local_context.clone(),
                    &local_source,
                    definition,
                ) {
                    Ok(Some(handle)) => local_context.global_state.threads().join_one(handle.id()),
                    Ok(None) => Ok(()),
                    Err(e) => Err(e),
                },
            )?))
        }
    }
}

pub fn eval_command_definition(
    command: &ValueDefinition,
    arguments: &Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
    definition: String,
) -> CrushResult<Option<ThreadHandle>> {
    match command.eval(&mut EvalContext::from(&context)) {
        Ok((this, value)) => {
            let local_arguments = arguments.clone();
            match value {
                Value::Command(command) => {
                    eval_command(command, this, local_arguments, context, source, definition)
                }
                Value::Type(t) => eval_type(t, local_arguments, context, source, definition),
                Value::Struct(s) => eval_struct(s, local_arguments, context, source, definition),
                // Only treat a File value as something to cd into or execute when it was
                // *written* as a bareword/path in command position (Identifier or a literal
                // Value token) -- this is what makes `./foo` at the head of a job run `foo`.
                // A File that merely *results* from evaluating some other expression here
                // (e.g. `GetAttr`, member access like `$s.x`) must not trigger execution,
                // or any struct/dict/row field holding a File would get silently run as a
                // subprocess the moment it's read. This list is deliberately an allow-list,
                // not a `!matches!(.., GetAttr(..))` deny-list: execution is the dangerous
                // side, so a future ValueDefinition variant should fail closed (never
                // execute) by default, not fail open.
                Value::File(s)
                    if matches!(
                        command,
                        ValueDefinition::Identifier(_, _) | ValueDefinition::Value(_, _)
                    ) =>
                {
                    if s.is_dir() && arguments.len() == 0 {
                        change_directory(s.to_path_buf(), command.source(), arguments.clone(), context)
                    } else {
                        external_command(s.to_path_buf(), command.source(), arguments.clone(), context)
                    }
                }
                v => {
                    if arguments.len() == 0 {
                        context.output.send(v)?;
                        Ok(None)
                    } else {
                        argument_error("Not a command", command.source())
                    }
                },
            }
        }
        Err(err) => {
            // Only a *bare* command name (`pwd`, promoted to a command lookup by
            // command-position syntax -- see `Node::compile`'s `Node::String` branch)
            // falls back to an external process when scope lookup fails, matching a
            // plain shell's own willingness to run anything found on `$PATH`. An
            // explicit `$` sigil (`$pwd`) is a deliberate, unambiguous request for a
            // real binding -- falling back silently to some same-named external
            // program would be surprising and could even be a security concern (e.g.
            // a typo'd or shadowed builtin silently running an attacker-controlled
            // binary off `$PATH` instead of erroring).
            if let ValueDefinition::Identifier(str, false) = command {
                try_external_command(str, arguments.clone(), context)
            } else {
                Err(err)
            }
        }
    }
}

fn eval_type(
    value_type: ValueType,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
    definition: String,
) -> CrushResult<Option<ThreadHandle>> {
    match value_type.fields().get("__call__") {
        None => {
            if local_arguments.len() == 0 {
                context.output.send(Value::Type(value_type))?;
                Ok(None)
            } else {
                command_error(format!(
                    "The {} type can't be subtyped.",
                    value_type
                ))
            }
        }
        Some(call) => eval_command(
            call.clone(),
            Some(Value::Type(value_type)),
            local_arguments,
            context,
            source,
            definition,
        ),
    }
}

fn eval_struct(
    struct_value: Struct,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
    definition: String,
) -> CrushResult<Option<ThreadHandle>> {
    match struct_value.get("__call__") {
        Some(Value::Command(call)) => eval_command(
            call,
            Some(Value::Struct(struct_value)),
            local_arguments,
            context,
            source,
            definition,
        ),

        Some(v) => error(
            format!(
                "Member `__call__` must be a command for struct to be callable, was of type {}",
                v.value_type().to_string()
            )
            .as_str(),
        ),

        // A struct with no `__call__`, given no arguments, is just a value -- exactly
        // like every other type in that case (see `eval_command_definition`'s final
        // catch-all arm). Only a struct *given arguments* without a `__call__` is an
        // actual "not a command" mistake.
        None if local_arguments.is_empty() => {
            context.output.send(Value::Struct(struct_value))?;
            Ok(None)
        }

        None => error(
            format!(
                "Struct must have a member `__call__` to be used as a command {}",
                struct_value.to_string()
            )
            .as_str(),
        ),
    }
}

fn eval_command(
    command: Command,
    this: Option<Value>,
    local_arguments: Vec<ArgumentDefinition>,
    context: JobContext,
    source: &Source,
    definition: String,
) -> CrushResult<Option<ThreadHandle>> {
    if !command.might_block(&local_arguments, &mut EvalContext::from(&context))
        && !arg_can_block(&local_arguments, &mut EvalContext::from(&context))
    {
        let new_context =
            CommandInvocation::command_context(source, local_arguments, this, context.clone())?;
        // Recorded even for a command that never gets its own thread: `commands`
        // joins this against `ThreadStore::current_threads()` (see that builtin's own
        // doc comment), so a short-lived, non-blocking command simply never shows up
        // there before it finishes -- same as a job finishing before a `jobs` listing
        // ever sees it.
        new_context
            .command_handle()
            .job_handle
            .set_command_info(new_context.command_handle().id, definition, command.kind());
        command.eval(new_context)?;
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
        command_context.command_handle().job_handle.set_command_info(
            command_context.command_handle().id,
            definition,
            command.kind(),
        );
        Ok(Some(context.global_state.threads().spawn(
            &name,
            &command_context.command_handle().clone(),
            move || command.eval(command_context),
        )?))
    }
}

pub fn resolve_external_command(name: &str) -> CrushResult<Option<PathBuf>> {
    let path_str = env::get("PATH")?;
    let path_vec: Vec<_> = path_str.split(':').collect();
    for i in path_vec {
        let Ok(val) = PathBuf::from_str(i);
        let full = val.join(name);
        if full.exists() {
            return Ok(Some(full));
        }
    }

    Ok(None)
}

/// Whether a `$PATH` entry is actually something a bare word in command position could
/// run. `resolve_external_command` itself only checks existence (`Path::exists`), so a
/// plain, non-executable regular file matches it too -- but running one fails at the OS
/// level (`EACCES`/"Permission denied"), so it isn't a valid command. A directory is
/// treated as valid here even though it isn't executable, mirroring how an explicit path
/// (e.g. `./some_dir`) is already a legitimate `cd` target elsewhere in the codebase.
/// Shared by `util::highlight` (to flag an unrunnable command red) and
/// `lang::completion` (to only offer real, runnable `$PATH` entries as completions).
pub fn is_runnable_as_command(path: &std::path::Path) -> bool {
    path.is_dir() || nix::unistd::access(path, nix::unistd::AccessFlags::X_OK).is_ok()
}

fn try_external_command(
    cmd: &Source,
    mut arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadHandle>> {
    match resolve_external_command(&cmd.str())? {
        None => {
            let name = cmd.str();
            let suggestions = suggest_similar_commands(&name, &context);
            let message = if suggestions.is_empty() {
                format!("Unknown command name `{}`", name)
            } else {
                format!(
                    "Unknown command name `{}`. Did you mean {}?",
                    name,
                    suggestions
                        .iter()
                        .map(|s| format!("`{}`", s))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            error(message.as_str())
        }
        Some(path) => {
            external_command(path, cmd, arguments, context)
        }
    }
}

/// The plain Levenshtein edit distance between `a` and `b` -- the number of single-
/// character inserts/deletes/substitutions needed to turn one into the other. Used by
/// `suggest_similar_commands` to rank candidates for an unknown command's "did you
/// mean" suggestions; a typo is typically 1-2 edits away from what was meant,
/// regardless of where in the name it landed, which neither a prefix nor a substring
/// check would reliably catch (e.g. `fsad` vs `fs`/`sad`/`cd`).
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for i in 1..=a.len() {
        curr[0] = i;
        for j in 1..=b.len() {
            curr[j] = if a[i - 1] == b[j - 1] {
                prev[j - 1]
            } else {
                1 + prev[j - 1].min(prev[j]).min(curr[j - 1])
            };
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Every runnable `$PATH` entry's own name (deduplicated, same "first match on `$PATH`
/// wins" rule a real lookup would use) -- the external half of `suggest_similar_
/// commands`'s candidate list. Uses the real `directory_lister()`, not an injectable
/// one: unlike completion's own `complete_path_entries`, this only ever runs on the
/// already-failed, real-process error path, with nothing to fake in a test.
fn external_command_names() -> Vec<String> {
    let mut seen = HashSet::new();
    let mut names = Vec::new();
    let Ok(path_env) = env::get("PATH") else {
        return names;
    };
    let lister = directory_lister();
    for dir in path_env.split(':') {
        if dir.is_empty() {
            continue;
        }
        let Ok(entries) = lister.list(dir) else {
            continue;
        };
        for entry in entries {
            let Some(name) = entry.name.to_str() else {
                continue;
            };
            if !is_runnable_as_command(&entry.full_path) || !seen.insert(name.to_string()) {
                continue;
            }
            names.push(name.to_string());
        }
    }
    names
}

/// Up to 3 command names -- drawn from both everything currently callable by a bare
/// word (every builtin and user-defined function visible from `context.scope`, the
/// same set `help`/tab-completion already see) and every runnable `$PATH` entry (the
/// same set a real external-command lookup checks) -- close enough to `typed` to be
/// worth suggesting as "did you mean" for an otherwise-unknown command. The distance
/// cutoff scales with `typed`'s own length: a one-letter name tolerates no typos at
/// all (anything else is a coincidence, not a typo), while a long name can plausibly
/// have 2-3 characters wrong and still clearly be "that one, misspelled".
fn suggest_similar_commands(typed: &str, context: &JobContext) -> Vec<String> {
    let mut candidates: Vec<String> = Vec::new();
    if let Ok(dump) = context.scope.dump() {
        for (name, value_type) in dump.iter() {
            if matches!(value_type, ValueType::Command) {
                candidates.push(name.clone());
            }
        }
    }
    candidates.extend(external_command_names());

    let max_distance = ((typed.chars().count() / 3) + 1).min(3);
    let mut scored: Vec<(usize, String)> = candidates
        .into_iter()
        .map(|c| (edit_distance(typed, &c), c))
        .filter(|(distance, name)| *distance > 0 && *distance <= max_distance && name != typed)
        .collect();
    scored.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    scored.dedup_by(|a, b| a.1 == b.1);
    scored.into_iter().take(3).map(|(_, name)| name).collect()
}

fn external_command(
    path: PathBuf,
    source: &Source,
    mut arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadHandle>> {
    arguments.insert(
        0,
        ArgumentDefinition::unnamed(ValueDefinition::Value(Value::from(path), source.clone())),
    );
    let call = CommandInvocation {
        command: ValueDefinition::Value(
            Value::Command(
                context
                    .scope
                    .global_static_cmd(vec!["global", "control", "cmd"])?,
            ),
            source.clone(),
        ),
        arguments,
        source: source.clone(),
    };
    call.eval(context)
}

fn change_directory(
    path: PathBuf,
    source: &Source,
    mut arguments: Vec<ArgumentDefinition>,
    context: JobContext,
) -> CrushResult<Option<ThreadHandle>> {
    arguments.insert(
        0,
        ArgumentDefinition::unnamed(ValueDefinition::Value(Value::from(path), source.clone())),
    );
    let call = CommandInvocation {
        command: ValueDefinition::Value(
            Value::Command(
                context
                    .scope
                    .global_static_cmd(vec!["global", "fs", "cd"])?,
            ),
            source.clone(),
        ),
        arguments,
        source: source.clone(),
    };
    call.eval(context)
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
