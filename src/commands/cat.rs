use crate::{
    data::{
        Argument,
        Value,
        ValueType,
    },
    errors::JobError,
    stream::{InputStream, OutputStream},
};
use crate::commands::command_util::{find_field_from_str, find_field};
use crate::commands::CompileContext;
use crate::data::ColumnType;
use crate::errors::{argument_error, error};
use crate::errors::JobResult;
use crate::replace::Replace;

pub struct Config {
    column: usize,
}

fn parse(input_type: &Vec<ColumnType>, arguments: &Vec<Argument>) -> Result<Config, JobError> {
    let indices: Vec<usize> = input_type
        .iter()
        .enumerate()
        .filter(|(_i, t)| match t.cell_type.clone() {
            ValueType::Output(_) | ValueType::Rows(_) => true,
            _ => false,
        })
        .map(|(i, _t)| i)
        .collect();
    return match arguments.len() {
        0 => match indices.len() {
            0 => Err(argument_error("No table-type column found")),
            1 => Ok(Config { column: indices[0] }),
            _ => Err(argument_error("Multiple table-type columns found")),
        },
        1 => match &arguments[0].value {
            Value::Text(s) => {
                let idx = find_field_from_str(s, &input_type)?;
                if indices.contains(&idx) { Ok(Config { column: idx }) } else { Err(argument_error("Field is not of table-type")) }
            }
            Value::Field(s) => {
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
                match row.cells.replace(config.column, Value::Integer(0)) {
                    Value::Stream(o) => loop {
                        match o.stream.recv() {
                            Ok(row) => {
                                output.send(row);
                            }
                            Err(_) => break,
                        }
                    }
                    Value::Rows(rows) => {
                        for row in rows.rows {
                            output.send(row)?;
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
        ValueType::Output(o) | ValueType::Rows(o) => Ok(o.clone()),
        _ => Err(argument_error("Invalid column")),
    }
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize_stream()?;
    let cfg = parse(input.get_type(), &context.arguments)?;
    let output_type = get_sub_type(&input.get_type()[cfg.column])?;
    let output = context.output.initialize(output_type)?;
    run(cfg, input, output)
}
