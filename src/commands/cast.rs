use crate::commands::CompileContext;
use crate::errors::JobResult;
use std::iter::Iterator;
use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    data::{
        Argument,
        Row,
        CellDefinition,
        Cell,
    },
    stream::{OutputStream, InputStream},
    replace::Replace,
};
use crate::data::{CellType, ColumnType};
use crate::printer::Printer;
use crate::env::Env;

pub struct Config {
    output_type: Vec<ColumnType>,
}

fn parse(
    input_type: &Vec<ColumnType>,
    arguments: &Vec<Argument>,
) -> Result<Config, JobError> {
    let mut output_type: Vec<ColumnType> = input_type.clone();
    for arg in arguments.iter() {
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
    })
}

pub fn run(
    config: Config,
    input: InputStream,
    output: OutputStream,
    printer: Printer,
) -> JobResult<()> {
    'outer: loop {
        match input.recv() {
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
                output.send(Row { cells });
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;
    let cfg = parse(input.get_type(), &context.arguments)?;
    let output = context.output.initialize(cfg.output_type.clone())?;
    run(cfg, input, output, context.printer)
}
