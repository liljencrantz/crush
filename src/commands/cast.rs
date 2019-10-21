use std::iter::Iterator;
use crate::{
    commands::command_util::find_field,
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellType,
        Cell
    },
    stream::{OutputStream, InputStream},
    replace::Replace
};
use crate::data::CellDataType;
use crate::printer::Printer;
use crate::state::State;

struct Config {
    output_type: Vec<CellType>,
}

fn parse(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Config, JobError> {

    let mut output_type: Vec<CellType> = input_type.clone();
    for (idx, arg) in arguments.iter().enumerate() {
        let arg_idx = match &arg.name {
            Some(name) => find_field(name, input_type)?,
            None => return Err(argument_error("Expected only named arguments")),
        };
        match &arg.cell {
            Cell::Text(s) => output_type[arg_idx].cell_type = CellDataType::from(s)?,
            _ => return Err(argument_error("Expected argument type as text field")),
        }
    }
    Ok(Config {
        output_type,
    })
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    state: State,
    printer: Printer,
) -> Result<(), JobError> {
    let cfg = parse(&input_type, &arguments)?;
    'outer: loop {
        match input.recv() {
            Ok(mut row) => {
                let mut cells = Vec::new();
                'inner: for (idx, cell) in row.cells.drain(..).enumerate() {
                    match cell.cast(cfg.output_type[idx].cell_type.clone()) {
                        Ok(c) => cells.push(c),
                        Err(e) => {
                            printer.job_error(e);
                            continue 'outer;
                        }
                    }
                }
                output.send(Row{cells});
            }
            Err(_) => break,
        }
    }
    return Ok(());
}

pub fn cast(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let cfg = parse(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("cast"),
        output_type: cfg.output_type,
        input_type,
        arguments,
        exec: Exec::Command(run),
    });
}
