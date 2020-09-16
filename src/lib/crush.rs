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
        e.insert(Value::String(key), Value::String(value));
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

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "crush",
        Box::new(move |crush| {
            crush.declare("pid", Value::Integer(Pid::this().as_raw() as i128))?;
            crush.declare("ppid", Value::Integer(Pid::parent().as_raw() as i128))?;
            crush.declare("env", make_env())?;
            Threads::declare(crush)?;

            Ok(())
        }),
    )?;
    Ok(())
}
