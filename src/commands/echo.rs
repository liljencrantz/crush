use crate::{
    data::CellType,
    stream::{OutputStream, InputStream},
    data::Row,
    data::Argument,
    commands::{Call, Exec},
    errors::JobError
};
use crate::printer::Printer;
use crate::env::Env;

fn run(
    _input_type: Vec<CellType>,
    mut arguments: Vec<Argument>,
    _input: InputStream,
    output: OutputStream,
    env: Env,
    printer: Printer,
) -> Result<(), JobError> {
    output.send(Row {
        cells: arguments.drain(..).map(|c| c.cell).collect()
    })
}

pub fn echo(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let output_type = arguments.iter().map(Argument::cell_type).collect();
    return Ok(Call {
        name: String::from("echo"),
        input_type,
        arguments,
        output_type,
        exec: Exec::Command(run),
    });
}
