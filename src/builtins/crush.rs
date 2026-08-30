use chrono::{Duration, Local};
use crate::builtins::term::{CYAN, GREEN, MAGENTA, RED, YELLOW};
use crate::data::list::List;
use crate::lang::ast::lexer::LanguageMode;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::data::dict::Dict;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{command_error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::global_state::RunMode;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use nix::unistd::Pid;
use rand::Rng;
use rustyline::history::{History, SearchDirection};
use signature::signature;
use crate::lang::state::id::JobId;

mod env {
    use crate::lang::command::OutputType::Known;
    use crate::lang::data::table::{ColumnType, Row};
    use crate::lang::errors::{CrushResult, command_error};
    use crate::lang::state::contexts::CommandContext;
    use crate::lang::value::Value;
    use crate::lang::value::ValueType;
    use signature::signature;
    use crate::util::env;

    #[signature(
    __getitem__,
    output = Known(ValueType::String),
    short = "Gets the variable with the given name",
    )]
    pub(crate) struct GetItem {
        #[description("The name of the environment variable to get")]
        name: String,
    }

    fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
        let cfg: GetItem =
            GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
        context.output.send(Value::from(env::get(&cfg.name)?))
    }

    #[signature(
    __setitem__,
    output = Known(ValueType::Empty),
    short = "Gets the variable with the given name",
    )]
    pub(crate) struct SetItem {
        #[description("The name of the environment variable to set")]
        name: String,
        #[description("The value to set the environment variable to")]
        value: String,
    }

    fn __setitem__(mut context: CommandContext) -> CrushResult<()> {
        let cfg: SetItem =
            SetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
        env::set(&cfg.name, &cfg.value)?;
        context.output.send(Value::Empty)
    }

    static LIST_OUTPUT_TYPE: [ColumnType; 2] = [
        ColumnType::new("name", ValueType::String),
        ColumnType::new("value", ValueType::String),
    ];

    #[signature(
    list,
    output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
    short = "Returns all environment variables and their values",
    )]
    pub(crate) struct List {}

    fn list(context: CommandContext) -> CrushResult<()> {
        let output = context.initialize_output(&LIST_OUTPUT_TYPE)?;
        for (k, v) in env::list() {
            output.send(Row::new(vec![Value::from(k), Value::from(v)]))?;
        }
        Ok(())
    }
}

fn make_arguments() -> Value {
    List::new(
        ValueType::String,
        std::env::args().map(|a| Value::from(a)).collect::<Vec<_>>(),
    )
    .into()
}

static THREADS_OUTPUT_TYPE: [ColumnType; 4] = [
    ColumnType::new("job_id", ValueType::Integer),
    ColumnType::new("command_id", ValueType::Integer),
    ColumnType::new("created", ValueType::Time),
    ColumnType::new("name", ValueType::String),
];

#[signature(
    crush.threads,
    output = Known(ValueType::table_input_stream(&THREADS_OUTPUT_TYPE)),
    short = "All the subthreads crush is currently running."
)]
struct Threads {}

fn threads(context: CommandContext) -> CrushResult<()> {
    let output = context.initialize_output(&THREADS_OUTPUT_TYPE)?;

    for t in context.global_state.threads().current_threads()? {
        output.send(Row::new(vec![
            Value::from(t.job_id),
            Value::from(t.command_id),
            Value::Time(t.creation_time),
            Value::from(t.name),
        ]))?;
    }
    Ok(())
}

#[signature(
    crush.exit,
    output = Known(ValueType::Empty),
    short = "Exit the shell",
    long = "Crush will not actually exit until all jobs have finished.",
)]
struct Exit {
    #[default(0)]
    #[description("The exit status to set for the process")]
    status: i32,
    #[default(false)]
    #[description("Terminate all running jobs")]
    force: bool,
}

fn random_other_job(context: &CommandContext) -> Option<JobId> {
    let my_job_id = context.command_handle().job_handle.id();
    let other_jobs : Vec<_> = context.global_state.jobs().drain(..).filter(|job| {job.id != my_job_id}).collect();
    match other_jobs.is_empty() {
        true => None,
        false => Some(other_jobs[rand::rng().random_range(0..other_jobs.len())].id),
    }
}

fn exit(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Exit = Exit::parse(context.remove_arguments(), &context.global_state.printer())?;

    if random_other_job(&context).is_some() {
        if cfg.force {
            let stop_time = Local::now() + Duration::seconds(2);
            while let Some(job) = random_other_job(&context) {
                if Local::now() > stop_time {
                    return command_error("Failed to terminate jobs.");
                }
                let _ = context.global_state.terminate(job);
                context.global_state.threads().reap(context.global_state.printer());
            }
        } else {
            return command_error("There are running jobs.");
        }
    }

    context.scope.do_exit()?;
    context.global_state.set_exit_status(cfg.status as i32);
    context.output.send(Value::Empty)
}

#[signature(
    crush.terminate,
    output = Known(ValueType::Empty),
    short = "Terminate the given job.",
    long = "A job may continue running for some time after receiving a termination notification. Output pipelines produced by the job will still be readable and will contain any buffered IO already sent to the job before it received the termination notification.",
    example = "# Create a job that produces a lot of output",
    example = "$all_files := $(files --recurse /)",
    example = "# Read a few lines of output",
    example = "$all_files | head",
    example = "# Find the job id of the job we want to terminate",
    example = "crush:jobs",
    example = "# Ask the job to terminate.",
    example = "crush:terminate 1",
    example = "# This will output the number of buffered rows that were buffered",
    example = "$all_files | count",
)]
struct Terminate {
    #[description("The job id for the job to terminate")]
    jid: usize,
}

fn terminate(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Terminate::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.global_state.terminate(cfg.jid.into())?;
    context.output.send(Value::Empty)
}

#[signature(
    crush.pause,
    output = Known(ValueType::Empty),
    short = "Pause the given job.",
)]
struct Pause {
    #[description("The job id for the job to pause")]
    jid: usize,
}

fn pause(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Pause::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.global_state.pause(cfg.jid.into())?;
    context.output.send(Value::Empty)
}

#[signature(
    crush.resume,
    output = Known(ValueType::Empty),
    short = "Pause the given job.",
)]
struct Resume {
    #[description("The job id for the job to resume")]
    jid: usize,
}

fn resume(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Resume::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.global_state.resume(cfg.jid.into())?;
    context.output.send(Value::Empty)
}

mod prompt {
    use super::*;

    #[signature(
        crush.prompt.set,
        can_block = false,
        short = "Set a new prompt command.",
        output = Known(ValueType::Empty)
    )]
    pub struct Set {
        #[description("The new command to invoke in order to produce a prompt")]
        prompt: Option<Command>,
    }

    fn set(mut context: CommandContext) -> CrushResult<()> {
        let cfg: Set = Set::parse(context.remove_arguments(), &context.global_state.printer())?;
        context.global_state.set_prompt(cfg.prompt);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.prompt.get,
        can_block = false,
        short = "Get the current prompt command.")
    ]
    pub struct Get {}

    fn get(mut context: CommandContext) -> CrushResult<()> {
        Get::parse(context.remove_arguments(), &context.global_state.printer())?;
        context.output.send(
            context
                .global_state
                .prompt()
                .map(|cmd| Value::Command(cmd))
                .unwrap_or(Value::Empty),
        )
    }
}

#[signature(
    crush.language_mode,
    can_block = false,
    output = Known(ValueType::String),
    short = "Returns the current language mode, either `command` or `expression`.",
    long = "Command mode is the default mode.",
)]
pub struct LanguageModeArg {}

fn language_mode(context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::from(match context.global_state.language_mode() {
            LanguageMode::Command => "command",
            LanguageMode::Expression => "expression",
        }))
}
#[signature(
    crush.run_mode,
    can_block = false,
    output = Known(ValueType::String),
    short = "Returns how crush is currently running, either `interactive` or `non-interactive`.",
    long = "In interactive mode, the prompt is shown and commands are entered interactively with access to history, keyboard shortcuts, etc. In non-interactive mode, no prompt is shown and commands are read from a file.",
    long = "The run mode can not be changed while crush is running. It is decided by how crush was started.",
)]
pub struct RunModeArg {}

fn run_mode(context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::from(match context.global_state.run_mode() {
            RunMode::Interactive => "interactive",
            RunMode::NonInteractive => "non-interactive",
        }))
}

mod title {
    use super::*;

    #[signature(
        crush.title.set,
        can_block = false,
        short = "Set a new title command",
        output = Known(ValueType::Empty)
    )]
    pub struct Set {
        #[description("The new command to invoke in order to produce a title")]
        title: Option<Command>,
    }

    fn set(mut context: CommandContext) -> CrushResult<()> {
        let cfg: Set = Set::parse(context.remove_arguments(), &context.global_state.printer())?;
        context.global_state.set_title(cfg.title);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.title.get,
        can_block = false,
        short = "Get the current title command")
    ]
    pub struct Get {}

    fn get(mut context: CommandContext) -> CrushResult<()> {
        Get::parse(context.remove_arguments(), &context.global_state.printer())?;
        context.output.send(
            context
                .global_state
                .title()
                .map(|cmd| Value::Command(cmd))
                .unwrap_or(Value::Empty),
        )
    }
}

static JOB_OUTPUT_TYPE: [ColumnType; 4] = [
    ColumnType::new("id", ValueType::Integer),
    ColumnType::new("description", ValueType::String),
    ColumnType::new("type", ValueType::String),
    ColumnType::new("status", ValueType::String),
];

#[signature(
    crush.jobs,
    can_block = false,
    short = "List running jobs",
    output = Known(ValueType::table_input_stream(&JOB_OUTPUT_TYPE)),
    long = "All currently running jobs")]
struct Jobs {}

fn jobs(context: CommandContext) -> CrushResult<()> {
    let output = context.initialize_output(&JOB_OUTPUT_TYPE)?;
    let jobs = context.global_state.jobs();
    for job in jobs {
        output.send(Row::new(vec![
            Value::from(job.id),
            Value::from(job.description),
            Value::from(job.job_type.to_string()),
            Value::from(job.status.to_string()),
        ]))?;
    }
    Ok(())
}

static HISTORY_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("idx", ValueType::Integer),
    ColumnType::new("command", ValueType::String),
];

#[signature(
    crush.history,
    can_block = true,
    short = "List previous commands",
    output = Known(ValueType::table_input_stream(&HISTORY_OUTPUT_TYPE)),
    long = "All previous invocation")]
struct HistoryCommand {}

fn history(context: CommandContext) -> CrushResult<()> {
    let output = context.initialize_output(&HISTORY_OUTPUT_TYPE)?;
    let mut res = Vec::new();
    context.global_state.editor().as_mut().map(|editor| {
        let history = editor.history();
        for i in 0..(history.len()) {
            if let Ok(Some(c)) = history.get(i, SearchDirection::Reverse) {
                res.push(c.entry.to_string());
            }
        }
    });
    let len = res.len();
    for (idx, c) in res.into_iter().enumerate() {
        output.send(Row::new(vec![Value::from(len - idx), Value::from(c)]))?;
    }
    Ok(())
}

mod locale {
    use super::*;
    use crate::lang::completion::Completion;
    use crate::lang::completion::parse::{LastArgument, PartialCommandResult};
    use crate::util::escape::{escape, escape_without_quotes};
    use num_format::SystemLocale;

    static LIST_OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("name", ValueType::String)];

    #[signature(
        crush.locale.list,
        output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
        short = "List all available locales."
    )]
    pub struct List {}

    fn list(context: CommandContext) -> CrushResult<()> {
        let output = context.initialize_output(&LIST_OUTPUT_TYPE)?;
        let available = SystemLocale::available_names()?;

        for name in available {
            output.send(Row::new(vec![Value::from(name)]))?;
        }
        Ok(())
    }

    fn locale_complete(
        cmd: &PartialCommandResult,
        _cursor: usize,
        _scope: &Scope,
        res: &mut Vec<Completion>,
    ) -> CrushResult<()> {
        for name in SystemLocale::available_names()? {
            match &cmd.last_argument {
                LastArgument::Unknown => res.push(Completion::new(escape(&name), name, 0)),

                LastArgument::QuotedString(stripped_prefix) => {
                    if name.starts_with(stripped_prefix) && name.len() > 0 {
                        res.push(Completion::new(
                            format!(
                                "{}\" ",
                                escape_without_quotes(&name[stripped_prefix.len()..])
                            ),
                            name,
                            0,
                        ));
                    }
                }

                _ => {}
            }
        }
        Ok(())
    }

    #[signature(
        crush.locale.set, output = Known(ValueType::Empty), short = "Set the current locale."
    )]
    pub struct Set {
        #[custom_completion(locale_complete)]
        #[description("the new locale.")]
        locale: String,
    }

    fn set(mut context: CommandContext) -> CrushResult<()> {
        let config: Set = Set::parse(context.remove_arguments(), &context.global_state.printer())?;
        let new_locale = SystemLocale::from_name(config.locale)?;
        context.global_state.set_locale(new_locale);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.locale.get, output = Known(ValueType::String), short = "Get the current locale."
    )]
    pub struct Get {}

    fn get(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::from(
            context.global_state.format_data().locale().name(),
        ))
    }
}

mod byte_unit {
    use super::*;
    use crate::util::byte_unit::ByteUnit;

    static LIST_OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("name", ValueType::String)];

    #[signature(
        crush.byte_unit.list,
        output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
        short = "List all available locales."
    )]
    pub struct List {}

    fn list(context: CommandContext) -> CrushResult<()> {
        let output = context.initialize_output(&LIST_OUTPUT_TYPE)?;

        for name in ByteUnit::units() {
            output.send(Row::new(vec![Value::from(name.to_string())]))?;
        }
        Ok(())
    }

    #[signature(
        crush.byte_unit.set, output = Known(ValueType::Empty), short = "Set the current byte unit."
    )]
    pub struct Set {
        #[description("the new byte unit.")]
        byte_unit: String,
    }

    fn set(mut context: CommandContext) -> CrushResult<()> {
        let config: Set = Set::parse(context.remove_arguments(), &context.global_state.printer())?;
        let new = ByteUnit::try_from(config.byte_unit.as_str())?;
        context.global_state.set_byte_unit(new);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.byte_unit.get, output = Known(ValueType::String), short = "Get the current byte unit."
    )]
    pub struct Get {}

    fn get(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::from(
            context.global_state.format_data().byte_unit().to_string(),
        ))
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "crush",
        "Information about this Crush session",
        Box::new(move |crush| {
            crush.declare("pid", Value::Integer(Pid::this().as_raw() as i128))?;
            crush.declare("ppid", Value::Integer(Pid::parent().as_raw() as i128))?;

            let highlight = Dict::new(ValueType::String, ValueType::String)?;
            highlight.insert(Value::from("operator"), Value::from(CYAN))?;
            highlight.insert(Value::from("string_literal"), Value::from(CYAN))?;
            highlight.insert(Value::from("file_literal"), Value::from(CYAN))?;
            highlight.insert(Value::from("identifier"), Value::from(CYAN))?;
            highlight.insert(Value::from("numeric_literal"), Value::from(CYAN))?;
            highlight.insert(Value::from("glob_literal"), Value::from(CYAN))?;
            highlight.insert(Value::from("regex_literal"), Value::from(CYAN))?;
            highlight.insert(Value::from("command"), Value::from(GREEN))?;
            highlight.insert(Value::from("keyword"), Value::from(MAGENTA))?;
            highlight.insert(Value::from("error"), Value::from(RED))?;
            highlight.insert(Value::from("comment"), Value::from(YELLOW))?;
            crush.declare("highlight", highlight.into())?;

            crush.declare("arguments", make_arguments())?;

            crush.create_namespace(
                "prompt",
                "Prompt data for Crush",
                Box::new(move |env| {
                    prompt::Set::declare(env)?;
                    prompt::Get::declare(env)?;

                    Ok(())
                }),
            )?;

            RunModeArg::declare(crush)?;
            LanguageModeArg::declare(crush)?;
            Terminate::declare(crush)?;
            Pause::declare(crush)?;
            Resume::declare(crush)?;

            crush.create_namespace(
                "title",
                "Title data for Crush",
                Box::new(move |env| {
                    title::Set::declare(env)?;
                    title::Get::declare(env)?;
                    Ok(())
                }),
            )?;

            Threads::declare(crush)?;
            Exit::declare(crush)?;
            Jobs::declare(crush)?;
            HistoryCommand::declare(crush)?;

            crush.create_namespace(
                "locale",
                "Locale data for Crush",
                Box::new(move |env| {
                    locale::List::declare(env)?;
                    locale::Get::declare(env)?;
                    locale::Set::declare(env)?;
                    Ok(())
                }),
            )?;

            crush.create_namespace(
                "env",
                "Environment variables",
                Box::new(move |loader| {
                    env::GetItem::declare(loader)?;
                    env::SetItem::declare(loader)?;
                    env::List::declare(loader)?;
                    Ok(())
                }),
            )?;

            crush.create_namespace(
                "byte_unit",
                "Formating style for table columns containing byte sizes.",
                Box::new(move |env| {
                    byte_unit::List::declare(env)?;
                    byte_unit::Get::declare(env)?;
                    byte_unit::Set::declare(env)?;
                    Ok(())
                }),
            )?;
            Ok(())
        }),
    )?;
    Ok(())
}
