use crate::builtins::term::{CYAN, GREEN, MAGENTA, RED, YELLOW};
use crate::data::list::List;
use crate::lang::any_sequence::AnySequence;
use crate::lang::ast::lexer::LanguageMode;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::data::dict::Dict;
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{command_error, error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::global_state::RunMode;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use nix::unistd::Pid;
use rustyline::history::{History, SearchDirection};
use signature::signature;

mod env {
    use crate::lang::command::OutputType::Known;
    use crate::lang::data::table::{ColumnType, Row};
    use crate::lang::errors::CrushResult;
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
    short = "Sets the variable with the given name",
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

    #[signature(
    unset,
    output = Known(ValueType::Empty),
    short = "Removes the environment variables with the given names",
    example = "crush:env:unset \"FOO\" \"BAR\"",
    )]
    pub(crate) struct Unset {
        #[description("the names of the environment variables to remove")]
        #[unnamed()]
        name: Vec<String>,
    }

    fn unset(mut context: CommandContext) -> CrushResult<()> {
        let cfg: Unset =
            Unset::parse(context.remove_arguments(), &context.global_state.printer())?;
        for name in cfg.name {
            env::unset(&name)?;
        }
        context.output.send(Value::Empty)
    }

    static LIST_OUTPUT_TYPE: [ColumnType; 2] = [
        ColumnType::new("name", ValueType::String, "the environment variable's name."),
        ColumnType::new("value", ValueType::String, "the environment variable's value."),
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
    ColumnType::new("job_id", ValueType::Integer, "the id of the job this thread is running as part of."),
    ColumnType::new("command_id", ValueType::Integer, "the id of the specific command invocation this thread is running."),
    ColumnType::new("created", ValueType::Time, "when this thread was created."),
    ColumnType::new("name", ValueType::String, "the thread's name."),
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
        output = Known(ValueType::one_of(vec![ValueType::Command, ValueType::Empty])),
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
        output = Known(ValueType::one_of(vec![ValueType::Command, ValueType::Empty])),
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

static HISTORY_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("idx", ValueType::Integer, "the command's position in the history, counting back from the most recent."),
    ColumnType::new("command", ValueType::String, "the command line as it was entered."),
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

mod warn {
    use super::*;

    static LIST_OUTPUT_TYPE: [ColumnType; 5] = [
        ColumnType::new("timestamp", ValueType::Time, "when the warning was recorded."),
        ColumnType::new("command", ValueType::OneOf(AnySequence::Slice(&[ValueType::String, ValueType::Empty])), "the name of the command that reported the warning, if known."),
        ColumnType::new("message", ValueType::String, "the warning message."),
        ColumnType::new("file", ValueType::OneOf(AnySequence::Slice(&[ValueType::String, ValueType::Empty])), "the source file the warning was reported from, if known."),
        ColumnType::new("location", ValueType::OneOf(AnySequence::Slice(&[ValueType::String, ValueType::Empty])), "the location within the source file the warning was reported from, if known."),
    ];

    #[signature(
        crush.warn.list,
        can_block = false,
        short = "List recent warnings reported by commands that experienced a partial failure.",
        output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
        long = "Commands that continue past a partial failure (e.g. one bad row out of a",
        long = "stream) report it as a warning rather than aborting. A bounded number of the",
        long = "most recent warnings are kept here (see `crush:warn:limit`); in interactive",
        long = "mode, they are also printed as soon as they happen, unless disabled (see",
        long = "`crush:warn:print`).",
    )]
    pub struct List {}

    fn list(context: CommandContext) -> CrushResult<()> {
        let output = context.initialize_output(&LIST_OUTPUT_TYPE)?;
        for w in context.global_state.warnings() {
            let (file, location) = match w.source() {
                Some(s) => (
                    s.file()
                        .map(|f| Value::from(f.to_string_lossy().to_string()))
                        .unwrap_or(Value::Empty),
                    Value::from(format!("{}-{}", s.location().start, s.location().end)),
                ),
                None => (Value::Empty, Value::Empty),
            };
            output.send(Row::new(vec![
                Value::Time(w.timestamp()),
                w.command().clone().map(Value::from).unwrap_or(Value::Empty),
                Value::from(w.message()),
                file,
                location,
            ]))?;
        }
        Ok(())
    }

    #[signature(
        crush.warn.new,
        can_block = false,
        output = Known(ValueType::Empty),
        short = "Report a warning to the bounded log crush:warn:list keeps.",
        long = "Every builtin that presses on past a partial failure (each/where/group/files,",
        long = "and others) reports it through this exact same mechanism instead of aborting;",
        long = "this is that mechanism made directly callable, for a script or closure that",
        long = "wants to flag something without stopping.",
        example = "crush:warn:new \"something looked off, continuing anyway\"",
    )]
    pub struct New {
        #[description("the warning message to report.")]
        message: String,
    }

    fn new(mut context: CommandContext) -> CrushResult<()> {
        let cfg: New = New::parse(context.remove_arguments(), &context.global_state.printer())?;
        context
            .global_state
            .warn(&error::<()>(cfg.message).unwrap_err());
        context.output.send(Value::Empty)
    }

    pub mod limit {
        use super::*;

        #[signature(
            crush.warn.limit.set,
            output = Known(ValueType::Empty),
            short = "Set how many warnings crush:warn:list keeps before evicting the oldest."
        )]
        pub struct Set {
            #[description("the new warning limit.")]
            limit: i128,
        }

        fn set(mut context: CommandContext) -> CrushResult<()> {
            let config: Set =
                Set::parse(context.remove_arguments(), &context.global_state.printer())?;
            if config.limit < 0 {
                return command_error("The warning limit can't be negative.");
            }
            context.global_state.set_warning_limit(config.limit as usize);
            context.output.send(Value::Empty)
        }

        #[signature(
            crush.warn.limit.get,
            output = Known(ValueType::Integer),
            short = "Get how many warnings crush:warn:list keeps before evicting the oldest."
        )]
        pub struct Get {}

        fn get(context: CommandContext) -> CrushResult<()> {
            context
                .output
                .send(Value::from(context.global_state.warning_limit() as i128))
        }
    }

    pub mod print {
        use super::*;

        #[signature(
            crush.warn.print.set,
            output = Known(ValueType::Empty),
            short = "Set whether warnings are printed to the screen as they happen.",
            long = "Only takes effect in interactive mode -- a warning is always recorded in",
            long = "`crush:warn:list` either way, this only controls whether it's also printed",
            long = "immediately. Defaults to `true`.",
            example = "crush:warn:print:set $false",
        )]
        pub struct Set {
            #[description("whether to print warnings to the screen as they happen.")]
            print: bool,
        }

        fn set(mut context: CommandContext) -> CrushResult<()> {
            let config: Set =
                Set::parse(context.remove_arguments(), &context.global_state.printer())?;
            context.global_state.set_warn_print(config.print);
            context.output.send(Value::Empty)
        }

        #[signature(
            crush.warn.print.get,
            output = Known(ValueType::Bool),
            short = "Get whether warnings are printed to the screen as they happen."
        )]
        pub struct Get {}

        fn get(context: CommandContext) -> CrushResult<()> {
            context
                .output
                .send(Value::Bool(context.global_state.warn_print()))
        }
    }
}

mod locale {
    use super::*;
    use crate::lang::completion::Completion;
    use crate::lang::completion::parse::{LastArgument, PartialCommandResult};
    use crate::util::escape::{escape, escape_without_quotes};
    use num_format::SystemLocale;

    static LIST_OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("name", ValueType::String, "the locale's name.")];

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

    static LIST_OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("name", ValueType::String, "the byte unit's name.")];

    #[signature(
        crush.byte_unit.list,
        output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
        short = "List all available byte units."
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
        Some(
            "The `crush` namespace holds state and controls for the running Crush process \
             itself, as distinct from state about the external world (`host`) or the data \
             flowing through a pipeline. This is where you configure the prompt and title, \
             the warning log and syntax highlighting, read and write OS environment \
             variables, and inspect running threads. Job control itself (`exit`, `jobs`, \
             `pause`, `resume`, `terminate`) lives in `control`. See `docs/config.md` for a \
             guided tour of what's configurable here."
                .to_string(),
        ),
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
            highlight.insert(Value::from("warning"), Value::from(YELLOW))?;
            highlight.insert(Value::from("comment"), Value::from(YELLOW))?;
            crush.declare("highlight", highlight.into())?;

            crush.declare("arguments", make_arguments())?;

            crush.create_namespace(
                "prompt",
                "Prompt data for Crush",
                None,
                Box::new(move |env| {
                    prompt::Set::declare(env)?;
                    prompt::Get::declare(env)?;

                    Ok(())
                }),
            )?;

            RunModeArg::declare(crush)?;
            LanguageModeArg::declare(crush)?;

            crush.create_namespace(
                "title",
                "Title data for Crush",
                None,
                Box::new(move |env| {
                    title::Set::declare(env)?;
                    title::Get::declare(env)?;
                    Ok(())
                }),
            )?;

            Threads::declare(crush)?;
            HistoryCommand::declare(crush)?;

            crush.create_namespace(
                "warn",
                "Warnings reported by commands that experienced a partial failure",
                None,
                Box::new(move |env| {
                    warn::New::declare(env)?;
                    warn::List::declare(env)?;
                    env.create_namespace(
                        "limit",
                        "How many warnings crush:warn:list keeps before evicting the oldest",
                        None,
                        Box::new(move |env| {
                            warn::limit::Get::declare(env)?;
                            warn::limit::Set::declare(env)?;
                            Ok(())
                        }),
                    )?;
                    env.create_namespace(
                        "print",
                        "Whether warnings are printed to the screen as they happen",
                        None,
                        Box::new(move |env| {
                            warn::print::Get::declare(env)?;
                            warn::print::Set::declare(env)?;
                            Ok(())
                        }),
                    )?;
                    Ok(())
                }),
            )?;

            crush.create_namespace(
                "locale",
                "Locale data for Crush",
                None,
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
                None,
                Box::new(move |loader| {
                    env::GetItem::declare(loader)?;
                    env::SetItem::declare(loader)?;
                    env::List::declare(loader)?;
                    env::Unset::declare(loader)?;
                    Ok(())
                }),
            )?;

            crush.create_namespace(
                "byte_unit",
                "Formating style for table columns containing byte sizes.",
                None,
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
