use crate::{
    data::Argument,
    commands::{Exec},
    errors::{JobError, argument_error},
    env::Env,
};
use crate::stream::{InputStream, OutputStream};
use crate::printer::Printer;
use crate::data::ColumnType;

pub struct Config {
    arguments: Vec<Argument>,
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    for arg in config.arguments {
        env.set(arg.name.unwrap().as_ref(), arg.cell)?;
    }
    return Ok(());
}

pub fn compile(input_type: Vec<ColumnType>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<ColumnType>), JobError> {
    for arg in arguments.iter() {
        if arg.val_or_empty().is_empty() {
            return Err(
                argument_error("Missing variable name")
            );
        }
    }
    Ok((Exec::Set(Config { arguments }), vec![]))
}
