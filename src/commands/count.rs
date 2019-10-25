use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    errors::{JobError, argument_error},
    commands::{Call, Exec},
    data::{
        Argument,
        Row,
        CellDefinition,
        CellType,
        Cell
    },
    stream::{OutputStream, InputStream},
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;

pub fn parse(input_type: Vec<ColumnType>) -> bool {
    for t in input_type.iter() {
        match t.cell_type {
            CellType::Output(_) => return true,
            CellType::Rows(_) => return true,
            _ => (),
        }
    }
    false
}

fn get_output_type(input_type: &Vec<ColumnType>) -> Vec<ColumnType> {
    let res: Vec<ColumnType> =  input_type.iter().map(|t|
        match t.cell_type {
            CellType::Output(_) => ColumnType { name: t.name.clone(), cell_type: CellType::Integer},
            _ => t.clone(),
        }).collect();
    return res;
}

fn count_rows(s: &InputStream) -> Cell {
    let mut res: i128 = 0;
    loop {
        match s.recv() {
            Ok(_) => res+= 1,
            Err(_) => break,
        }
    }
    return Cell::Integer(res);
}

pub fn run(
    has_streams: bool,
    input: InputStream,
    output: OutputStream,
) -> JobResult<()> {
    if has_streams {
        loop {
            match input.recv() {
                Ok(row) => {
                    let mut cells: Vec<Cell> = Vec::new();
                    for c in row.cells {
                        match &c {
                            Cell::JobOutput(o) => cells.push(count_rows(&o.stream)),
                            Cell::Rows(r) => cells.push(Cell::Integer(r.rows.len() as i128)),
                            _ => {
                                cells.push(c)
                            }
                        }
                    }
                    output.send(Row { cells })?;
                }
                Err(_) => break,
            }
        }
    } else {
        output.send(Row { cells: vec![count_rows(&input)]})?;
    }
    return Ok(());
}

pub fn compile(context: CompileContext) -> JobResult<(Exec, Vec<ColumnType>)> {
    let has_streams = parse(context.input_type.clone());
    let input = context.input;
    let output = context.output;
    let output_type = if has_streams {
        get_output_type(&context.input_type)
    } else {
        vec![ColumnType::named("count", CellType::Integer)]
    };

    Ok((Exec::Command(Box::from(move || run(has_streams, input, output))), output_type))
}
