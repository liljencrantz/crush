use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    errors::JobError,
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellDefinition,
        CellType,
        JobOutput,
        Cell,
    },
    stream::{OutputStream, InputStream},
};
use crate::errors::{argument_error, error};
use crate::commands::command_util::find_field;
use crate::replace::Replace;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;

pub struct Config {
    column: usize,
}

fn parse(input_type: &Vec<ColumnType>, arguments: &Vec<Argument>) -> Result<Config, JobError> {
    let indices: Vec<usize> = input_type
        .iter()
        .enumerate()
        .filter(|(i, t)| match t.cell_type.clone() {
            CellType::Output(_) | CellType::Rows(_) => true,
            _ => false,
        })
        .map(|(i, t)| i)
        .collect();
    return match arguments.len() {
        0 => match indices.len() {
            0 => Err(argument_error("No table-type column found")),
            1 => Ok(Config { column: indices[0] }),
            _ => Err(argument_error("Multiple table-type columns found")),
        },
        1 => match &arguments[0].cell {
            Cell::Field(s) | Cell::Text(s) => {
                let idx = find_field(s, &input_type)?;
                if indices.contains(&idx) { Ok(Config { column: idx }) } else { Err(argument_error("Field is not of table-type")) }
            }
            _ => Err(argument_error("Expected a field"))
        },
        _ => Err(argument_error("Expected zero or one arguments"))
    };
}

pub fn run(
    config: Config,
    input: InputStream,
           output: OutputStream,
) -> JobResult<()> {
    loop {
        match input.recv() {
            Ok(mut row) => {
                match row.cells.replace(config.column, Cell::Integer(0)) {
                    Cell::JobOutput(o) => loop {
                        match o.stream.recv() {
                            Ok(row) => {
                                output.send(row);
                            }
                            Err(_) => break,
                        }
                    }
                    Cell::Rows(mut rows) => {
                        for row in rows.rows {
                            output.send(row);
                        }
                    }
                    _ => return Err(error("Invalid data")),
                }
            }
            Err(_) => break,
        }
    }

    return Ok(());
}

pub fn get_sub_type(cell_type: &ColumnType) -> Result<Vec<ColumnType>, JobError> {
    match &cell_type.cell_type {
        CellType::Output(o) | CellType::Rows(o) => Ok(o.clone()),
        _ => Err(argument_error("Invalid column")),
    }
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input;
    let output = context.output;
    let cfg = parse(&context.input_type, &context.arguments)?;
    let output_type = get_sub_type(&context.input_type[cfg.column])?;
    Ok((Exec::Command(Box::from(move || run(cfg, input, output))), output_type))
}
