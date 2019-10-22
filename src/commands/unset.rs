use crate::{
    data::CellDefinition,
    data::Argument,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    env::Env,
};
use crate::data::{Cell, CellFnurp};
use crate::printer::Printer;
use crate::stream::{InputStream, OutputStream};
use crate::errors::JobResult;

pub struct Config {
    vars: Vec<Box<str>>,
}

fn parse(arguments: Vec<Argument>) -> JobResult<Config> {
    let mut vars = Vec::new();
    for arg in arguments.iter() {
        if let Cell::Text(s) = &arg.cell {
            if s.len() == 0 {
                return Err(argument_error("Illegal variable name"));
            } else {
                vars.push(s.clone());
            }
        } else {
            return Err(argument_error("Illegal variable name"));
        }
    }
    Ok(Config { vars })
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    for s in config.vars {
        env.remove(s.as_ref());
    }
    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    Ok((Exec::Unset(parse(arguments)?), vec![]))
}
