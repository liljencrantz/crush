use crate::stream::{OutputStream, InputStream};
use crate::cell::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{Call, Exec};
use crate::errors::JobError;
use crate::state::State;

fn run(
    _input_type: Vec<CellType>,
    _arguments: Vec<Argument>,
    _input: InputStream,
    output: OutputStream) -> Result<(), JobError> {
    return match std::env::current_dir() {
        Ok(os_dir) => {
            match os_dir.to_str() {
                Some(dir) => {
                    output.send(Row {
                        cells: vec![Cell::Text(String::from(dir))]
                    });
                }
                None => {}
            }
            Ok(())
        }
        Err(io_err) =>
            Err(JobError { message: io_err.to_string() }),
    };
}

pub(crate) fn pwd(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    return Ok(Call {
        name: String::from("pwd"),
        input_type: input_type,
        arguments: arguments,
        output_type: vec![CellType {
            name: String::from("directory"),
            cell_type: CellDataType::Text,
        }],
        exec: Exec::Run(run),
    });
}
