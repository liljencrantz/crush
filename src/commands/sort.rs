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
};
use crate::printer::Printer;

pub fn get_key(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<usize, JobError> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    if let Some(name) = &arguments[0].name {
        match (name.as_ref(), &arguments[0].cell) {
            ("key", Cell::Text(cell_name)) | ("key", Cell::Field(cell_name)) => {
                return find_field(cell_name, &input_type);
            }
            _ => {
                return Err(argument_error("Bad comparison key"));
            }
        }
    } else {
        return Err(argument_error("Expected comaprison key"));
    }
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    printer: Printer,
) -> Result<(), JobError> {
    let idx = get_key(&input_type, &arguments)?;
    let mut res: Vec<Row> = Vec::new();
    loop {
        match input.recv() {
            Ok(row) => res.push(row),
            Err(_) => break,
        }
    }

    res.sort_by(|a, b| a.cells[idx].partial_cmp(&b.cells[idx]).expect("OH NO!"));

    for row in res {
        output.send(row)?;
    }

    return Ok(());
}

pub fn sort(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    get_key(&input_type, &arguments)?;
    return Ok(Call {
        name: String::from("Sort"),
        output_type: input_type.clone(),
        input_type,
        arguments,
        exec: Exec::Run(run),
    });
}
