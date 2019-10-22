use crate::{
    data::Argument,
    data::{CellDefinition, CellType},
    data::Cell,
    commands::{Call, Exec},
    errors::JobError,
    env::Env
};
use crate::errors::to_job_error;
use crate::printer::Printer;
use crate::stream::{OutputStream, InputStream};
use crate::data::CellFnurp;
use std::path::Path;

pub struct Config {dir: Box<Path>}

pub fn run(config: Config, env: Env, printer: Printer) -> Result<(), JobError> {
    to_job_error(std::env::set_current_dir(config.dir))
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    match arguments.len() {
        0 => Ok((Exec::Cd(Config {dir: Box::from(Path::new("/"))}), vec![])),
        1 => {
            let dir = &arguments[0];
            match &dir.cell {
                Cell::Text(val) => Ok((Exec::Cd(Config {dir: Box::from(Path::new(val.as_ref()))}), vec![])),
                Cell::File(val) => Ok((Exec::Cd(Config {dir: val.clone()}), vec![])),
                _ => Err(JobError { message: String::from("Wrong parameter type, expected text") })
            }
        }
        _ => Err(JobError { message: String::from("Wrong number of arguments") })
    }
}
