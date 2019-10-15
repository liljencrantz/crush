use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row};
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error};
use std::iter::Iterator;
use crate::commands::head::get_line_count;
use std::collections::VecDeque;

fn run(
    _input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    let tot = get_line_count(&arguments)?;
    let mut q: VecDeque<Row> = VecDeque::new();
    loop {
        match input.recv() {
            Ok(row) => {
                if q.len() >= tot as usize {
                    q.pop_front();
                }
                q.push_back(row);
            }
            Err(_) => {
                for row in q.drain(..) {
                    output.send(row)?;
                }
                break;
            },
        }
    }
    return Ok(());
}

pub fn tail(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    get_line_count(&arguments)?;
    return Ok(Call {
        name: String::from("tail"),
        output_type: input_type.clone(),
        input_type,
        arguments: arguments,
        exec: Exec::Run(run),
    });
}
