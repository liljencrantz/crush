use nix::unistd::Pid;
use signature::signature;
use std::env;
use rustyline::history::{History, SearchDirection};
use crate::lang::errors::{CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::command::OutputType::Known;
use crate::lang::command::Command;
use crate::data::list::List;
use crate::lang::data::dict::Dict;

fn make_env() -> CrushResult<Value> {
    let e = Dict::new(ValueType::String, ValueType::String)?;
    for (key, value) in env::vars() {
        let _ = e.insert(Value::from(key), Value::from(value));
    }
    Ok(e.into())
}

fn make_arguments() -> Value {
    List::new(ValueType::String, env::args().map(|a| { Value::from(a) }).collect::<Vec<_>>()).into()
}

static THREADS_OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new("jid", ValueType::Any),
    ColumnType::new("created", ValueType::Time),
    ColumnType::new("name", ValueType::String),
];

#[signature(
    crush.threads,
    output = Known(ValueType::table_input_stream(&THREADS_OUTPUT_TYPE)),
    short = "All the subthreads crush is currently running"
)]
struct Threads {}

fn threads(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&THREADS_OUTPUT_TYPE)?;

    for t in context.global_state.threads().current()? {
        output.send(Row::new(vec![
            t.job_id.map(|i| { Value::from(i) }).unwrap_or(Value::Empty),
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
}

fn exit(context: CommandContext) -> CrushResult<()> {
    let cfg: Exit = Exit::parse(context.arguments, &context.global_state.printer())?;
    context.scope.do_exit()?;
    context.global_state.set_exit_status(cfg.status as i32);
    context.output.send(Value::Empty)
}

mod prompt {
    use super::*;

    #[signature(
        crush.prompt.set,
        can_block = false,
        short = "Set a new prompt command",
        output = Known(ValueType::Empty)
    )]
    pub struct Set {
        #[description("The new command to invoke in order to produce a prompt")]
        prompt: Option<Command>,
    }

    fn set(context: CommandContext) -> CrushResult<()> {
        let cfg: Set = Set::parse(context.arguments, &context.global_state.printer())?;
        context.global_state.set_prompt(cfg.prompt);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.prompt.get,
        can_block = false,
        short = "Get the current prompt command")
    ]
    pub struct Get {
    }

    fn get(context: CommandContext) -> CrushResult<()> {
        Get::parse(context.arguments, &context.global_state.printer())?;
        context.output.send(context.global_state.prompt().map(|cmd| {Value::Command(cmd)}).unwrap_or(Value::Empty))
    }

    pub mod mode {
        use signature::signature;
        use crate::lang::ast::lexer::LexerMode;
        use crate::lang::errors::CrushResult;
        use crate::lang::state::contexts::CommandContext;
        use crate::lang::value::Value;
        use crate::lang::command::OutputType::Known;
        use crate::lang::value::ValueType;

        #[signature(
            crush.prompt.get,
            can_block = false,
            output = Known(ValueType::String),
            short = "Returns the current language mode")
        ]
        pub struct Get {}

        fn get(context: CommandContext) -> CrushResult<()> {
            context.output.send(Value::from(match context.global_state.mode() {
                LexerMode::Command => "command",
                LexerMode::Expression => "expression",
            }))
        }
    }

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

    fn set(context: CommandContext) -> CrushResult<()> {
        let cfg: Set = Set::parse(context.arguments, &context.global_state.printer())?;
        context.global_state.set_title(cfg.title);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.title.get,
        can_block = false,
        short = "Get the current title command")
    ]
    pub struct Get {
    }

    fn get(context: CommandContext) -> CrushResult<()> {
        Get::parse(context.arguments, &context.global_state.printer())?;
        context.output.send(context.global_state.title().map(|cmd| {Value::Command(cmd)}).unwrap_or(Value::Empty))
    }

}

static JOB_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("id", ValueType::Integer),
    ColumnType::new("description", ValueType::String),
];

#[signature(
    crush.jobs,
    can_block = false,
    short = "List running jobs",
    output = Known(ValueType::table_input_stream(&JOB_OUTPUT_TYPE)),
    long = "All currently running jobs")]
struct Jobs {}

fn jobs(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&JOB_OUTPUT_TYPE)?;
    for job in context.global_state.jobs() {
        output.send(Row::new(vec![
            Value::from(job.id),
            Value::from(job.description),
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
    let output = context.output.initialize(&HISTORY_OUTPUT_TYPE)?;
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
        output.send(Row::new(vec![
            Value::from(len - idx),
            Value::from(c),
        ]))?;
    }
    Ok(())
}

mod locale {
    use super::*;
    use num_format::SystemLocale;
    use crate::lang::completion::parse::{PartialCommandResult, LastArgument};
    use crate::lang::completion::Completion;
    use crate::util::escape::{escape, escape_without_quotes};

    static LIST_OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("name", ValueType::String)];

    #[signature(
        crush.locale.list,
        output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
        short = "List all available locales."
    )]
    pub struct List {}

    fn list(context: CommandContext) -> CrushResult<()> {
        let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
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
                LastArgument::Unknown => {
                    res.push(Completion::new(
                        escape(&name),
                        name,
                        0,
                    ))
                }

                LastArgument::QuotedString(stripped_prefix) => {
                    if name.starts_with(stripped_prefix) && name.len() > 0 {
                        res.push(Completion::new(
                            format!("{}\" ", escape_without_quotes(&name[stripped_prefix.len()..])),
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

    fn set(context: CommandContext) -> CrushResult<()> {
        let config: Set = Set::parse(context.arguments, &context.global_state.printer())?;
        let new_locale = SystemLocale::from_name(config.locale)?;
        context.global_state.set_locale(new_locale);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.locale.get, output = Known(ValueType::String), short = "Get the current locale."
    )]
    pub struct Get {}

    fn get(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::from(context.global_state.format_data().locale().name()))
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
        let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;

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

    fn set(context: CommandContext) -> CrushResult<()> {
        let config: Set = Set::parse(context.arguments, &context.global_state.printer())?;
        let new = ByteUnit::try_from(config.byte_unit.as_str())?;
        context.global_state.set_byte_unit(new);
        context.output.send(Value::Empty)
    }

    #[signature(
        crush.byte_unit.get, output = Known(ValueType::String), short = "Get the current byte unit."
    )]
    pub struct Get {}

    fn get(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::from(context.global_state.format_data().byte_unit().to_string()))
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
            highlight.insert(Value::from("operator"), Value::from(""))?;
            highlight.insert(Value::from("string_literal"), Value::from(""))?;
            highlight.insert(Value::from("file_literal"), Value::from(""))?;
            highlight.insert(Value::from("label"), Value::from(""))?;
            highlight.insert(Value::from("numeric_literal"), Value::from(""))?;
            highlight.insert(Value::from("glob_literal"), Value::from(""))?;
            highlight.insert(Value::from("regex_literal"), Value::from(""))?;
            highlight.insert(Value::from("command"), Value::from(""))?;
            highlight.insert(Value::from("keyword"), Value::from(""))?;
            crush.declare("highlight", highlight.into())?;

            crush.declare("env", make_env()?)?;
            crush.declare("arguments", make_arguments())?;

            crush.create_namespace(
                "prompt",
                "Prompt data for Crush",
                Box::new(move |env| {
                    prompt::Set::declare(env)?;
                    prompt::Get::declare(env)?;

                    env.create_namespace(
                        "mode",
                        "Crush language mode",
                        Box::new(move |env| {
                            prompt::mode::Get::declare(env)?;
                            Ok(())
                        }))?;
                    Ok(())
                }),
            )?;

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
