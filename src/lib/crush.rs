use crate::lang::errors::{CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::lang::data::table::{ColumnType, Row};
use signature::signature;
use crate::lang::command::OutputType::Known;
use nix::unistd::Pid;
use crate::lang::data::dict::Dict;
use std::env;
use lazy_static::lazy_static;

fn make_env() -> Value {
    let e = Dict::new(ValueType::String, ValueType::String);
    for (key, value) in env::vars() {
        let _ = e.insert(Value::String(key), Value::String(value));
    }
    Value::Dict(e)
}

lazy_static! {
    static ref THREADS_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("created", ValueType::Time),
        ColumnType::new("name", ValueType::String),
    ];
}

#[signature(threads, output = Known(ValueType::TableStream(THREADS_OUTPUT_TYPE.clone())), short = "All the subthreads crush is currently running")]
struct Threads {}

fn threads(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(THREADS_OUTPUT_TYPE.clone())?;

    for t in context.threads.current()? {
        output.send(Row::new(vec![
            Value::Time(t.creation_time),
            Value::String(t.name),
        ]))?;
    }
    Ok(())
}

mod locale {
    use super::*;
    use num_format::SystemLocale;
    use crate::lang::errors::to_crush_error;

    lazy_static! {
    static ref LIST_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("name", ValueType::String),
    ];
}

    #[signature(list, output = Known(ValueType::TableStream(LIST_OUTPUT_TYPE.clone())), short = "List all available locales.")]
    pub struct List {}

    fn list(context: CommandContext) -> CrushResult<()> {
        let output = context.output.initialize(LIST_OUTPUT_TYPE.clone())?;
        let mut available = to_crush_error(SystemLocale::available_names())?;

        for name in available {
            output.send(Row::new(vec![Value::String(name)]))?;
        }
        Ok(())
    }

    #[signature(set, output = Known(ValueType::Empty), short = "Set the current locale.")]
    pub struct Set {
        #[description("the new locale.")]
        locale: String,
    }

    fn set(context: CommandContext) -> CrushResult<()> {
        let config: Set = Set::parse(context.arguments, &context.printer)?;
        let new_locale = to_crush_error(SystemLocale::from_name(config.locale))?;
        context.global_state.set_locale(new_locale);
        context.output.send(Value::Empty())
    }

    #[signature(get, output = Known(ValueType::String), short = "Get the current locale.")]
    pub struct Get {}

    fn get(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::string(context.global_state.locale().name()))
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "crush",
        Box::new(move |crush| {
            crush.declare("pid", Value::Integer(Pid::this().as_raw() as i128))?;
            crush.declare("ppid", Value::Integer(Pid::parent().as_raw() as i128))?;
            crush.declare("env", make_env())?;
            Threads::declare(crush)?;

            crush.create_namespace(
                "locale",
                Box::new(move |env| {
                    locale::List::declare(env)?;
                    locale::Get::declare(env)?;
                    locale::Set::declare(env)?;
                    Ok(())
                }),
            )?;
            Ok(())
        }),
    )?;
    Ok(())
}
