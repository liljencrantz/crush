use std::iter::Iterator;
use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellDefinition,
        Cell,
    },
    stream::{OutputStream, InputStream},
    replace::Replace,
};
use crate::data::{CellType, CellFnurp};
use crate::printer::Printer;
use crate::env::Env;

pub struct Config {
    output_type: Vec<CellFnurp>,
    input: InputStream,
    output: OutputStream,
}

fn parse(
    input_type: &Vec<CellFnurp>,
    arguments: &Vec<Argument>,
    input: InputStream,
    output: OutputStream,
) -> Result<Config, JobError> {
    let mut output_type: Vec<CellFnurp> = input_type.clone();
    for (idx, arg) in arguments.iter().enumerate() {
        let arg_idx = match &arg.name {
            Some(name) => find_field(name, input_type)?,
            None => return Err(argument_error("Expected only named arguments")),
        };
        match &arg.cell {
            Cell::Text(s) => output_type[arg_idx].cell_type = CellType::from(s)?,
            _ => return Err(argument_error("Expected argument type as text field")),
        }
    }
    Ok(Config {
        output_type,
        input,
        output,
    })
}

pub fn run(
    config: Config,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    'outer: loop {
        match config.input.recv() {
            Ok(mut row) => {
                let mut cells = Vec::new();
                'inner: for (idx, cell) in row.cells.drain(..).enumerate() {
                    match cell.cast(config.output_type[idx].cell_type.clone()) {
                        Ok(c) => cells.push(c),
                        Err(e) => {
                            printer.job_error(e);
                            continue 'outer;
                        }
                    }
                }
                config.output.send(Row { cells });
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    let cfg = parse(&input_type, &arguments, input, output)?;
    let output_type = cfg.output_type.clone();
    Ok((Exec::Cast(cfg), output_type))
}
