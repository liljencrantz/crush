use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error, error};
use crate::{
    data::{
        Row,
        CellType,
        Cell
    },
    stream::{OutputStream, InputStream},
};
use crate::data::{ColumnType, Argument};
use either::Either;
use crate::commands::command_util::find_field_from_str;

pub fn parse(input_type: &Vec<ColumnType>, arguments: &Vec<Argument>) -> JobResult<(String, usize)> {
    if arguments.len() != 1 {
        return Err(error("Expected exactly one argument, a filed defintition"));
    }

    if let Cell::Field(f) = &arguments[0].cell {
        match f.len() {
            1 => {
                let idx = find_field_from_str(f[0].as_ref(), input_type)?;
                let name = if arguments[0].name.is_none() {
                    input_type[idx].name.as_ref().unwrap().to_string()
                } else {
                    arguments[0].name.as_ref().unwrap().to_string()
                };
                return Ok((name, idx));
            }
            _ => {
                return Err(error("Unexpectd field"));
            }
        }
    }
    return Err(error("Expected exactly one argument, a field definitition"));
}

fn count_rows(s: &InputStream, column: usize) -> JobResult<Cell> {
    let mut res: i128 = 0;
    loop {
        match s.recv() {
            Ok(row) => match row.cells[column] {
                Cell::Integer(i) => res += i,
                _ => return Err(error("Invalid cell value, expected an integer"))
            },
            Err(_) => break,
        }
    }
    Ok(Cell::Integer(res))
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;
    let (name, column) = parse(input.get_type(), &context.arguments)?;
    let output_type = vec![ColumnType::named(name.as_str(), CellType::Integer)];
    let output = context.output.initialize(output_type)?;
    output.send(Row { cells: vec![count_rows(&input, column)?]})
}
