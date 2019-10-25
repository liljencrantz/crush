use crate::{
    data::Argument,
    data::Row,
    data::{CellType},
    stream::{OutputStream, InputStream},
    data::Cell,
    commands::{Exec},
    errors::JobError,
    env::get_cwd
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;
use crate::errors::{argument_error, JobResult};
use crate::closure::ClosureDefinition;
use crate::commands::CompileContext;

pub struct Config {
    input_type: Vec<ColumnType>,
    closure: ClosureDefinition,
    output_type: Vec<ColumnType>,
}

pub fn parse(context: &CompileContext) -> Result<Config, JobError> {
    if context.arguments.len() != 2 {
        return Err(argument_error("Expected exactly two arguments"));
    }

    match (&context.arguments[0].cell, &context.arguments[1].cell) {
/*        (Cell::JobOutput(o), Cell::ClosureDefinition(c)) => {

            parent_env: &Env,
            printer: &Printer,
            initial_input_type: &Vec<ColumnType>,
            first_input: InputStream,
            last_output: OutputStream,
            arguments: Vec<Argument>

            c.compile();

            Ok(Config {
                input_type: o.types,
                input: o.stream,
                closure: c,
                output_type: c.,
                output
            })
        }*/
        _ => Err(argument_error("First argument to for must be a job, the second must be a closure")),
    }
}

pub fn run(
    config: Config,
    input: InputStream,
    output: OutputStream
) -> JobResult<()> {
//    output.send(Row { cells: vec![Cell::File(get_cwd()?)] })?;
    Ok(())
}

pub fn compile(context: CompileContext) -> JobResult<(Exec, Vec<ColumnType>)> {
    let config = parse(&context)?;
    let output_type = config.output_type.clone();
    return Ok((Exec::Command(Box::from(move ||run(config, context.input, context.output))), output_type))
}
