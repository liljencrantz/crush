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

pub fn get_key(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<usize, JobError> {
    if arguments.len() != 1 {
        return Err(argument_error("No comparison key specified"));
    }
    match (arguments[0].name.as_ref().map(|s| s.as_str()), &arguments[0].cell) {
        (Some("key"), Cell::Text(cell_name)) | (Some("key"), Cell::Field(cell_name))=> {
            return find_field(cell_name, &input_type);
        }
        _ => {
            return Err(argument_error("Bad comparison key"));
        }
    }
}

fn run(
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
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
