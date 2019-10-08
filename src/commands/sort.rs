use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row};
use crate::commands::Call;
use crate::errors::{JobError, argument_error};
use crate::state::State;
use crate::commands::filter::find_field;

fn run(
    input_type: &Vec<CellType>,
    arguments: &Vec<Argument>,
    input: &mut InputStream,
    output: &mut OutputStream) -> Result<(), JobError> {
    match (arguments[0].name.as_str(), &arguments[0].cell) {
        ("key", Cell::Text(cell_name)) => {
            let idx = find_field(cell_name, input_type)?;
            let mut res: Vec<Row> = Vec::new();
            loop {
                match input.recv() {
                    Ok(row) => {
                        res.push(row);
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
            res.sort_by(|a, b| a.cells[idx].partial_cmp(&b.cells[idx]).expect("OH NO!"));
            for row in &res {
                output.send(row.clone());
            }

            return Ok(());
        }
        _ => {
            return Err(argument_error("Bad comparison key"));
        }
    }
}

pub fn sort(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("Sort"),
        input_type: input_type.clone(),
        arguments: arguments.clone(),
        output_type: input_type.clone(),
        run: Some(run),
        mutate: None,
    });
}
