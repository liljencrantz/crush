use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    data::Argument,
    commands::{Exec},
    errors::{JobError, argument_error},
    env::Env,
};
use crate::stream::{InputStream, OutputStream};
use crate::printer::Printer;
use crate::data::ColumnType;


pub fn run(
    arguments: Vec<Argument>,
    env: Env,
) -> JobResult<()> {
    for arg in arguments {
        env.set(arg.name.unwrap().as_ref(), arg.cell)?;
    }
    return Ok(());
}

pub fn compile(context: CompileContext) -> JobResult<(Exec, Vec<ColumnType>)> {
    for arg in context.arguments.iter() {
        if arg.val_or_empty().is_empty() {
            return Err(
                argument_error("Missing variable name")
            );
        }
    }
    Ok((Exec::Command(Box::from(move || run(context.arguments, context.env))), vec![]))
}
