use crate::{
    data::Row,
    data::{CellDefinition},
    stream::{OutputStream, InputStream},
    data::Argument,
    commands::{Call, Exec},
    errors::{JobError, argument_error},
    commands::head::get_line_count
};
use std::collections::VecDeque;
use crate::printer::Printer;
use crate::env::Env;
use crate::data::CellFnurp;

fn run(
    _input_type: Vec<CellFnurp>,
    arguments: Vec<Argument>,
    input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
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

pub fn compile(input_type: Vec<CellFnurp>, input: InputStream, output: OutputStream, arguments: Vec<Argument>) -> Result<(Exec, Vec<CellFnurp>), JobError> {
    get_line_count(&arguments)?;
    return Ok(Call {
        name: String::from("tail"),
        output_type: input_type.clone(),
        input_type,
        arguments: arguments,
        exec: Exec::Command(run),
    });
}
