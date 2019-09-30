use std::{io, fs};
use crate::stream::{OutputStream, InputStream};
use crate::result::{Argument, CellType, Cell, Row, CellDataType};
use crate::commands::{InternalCall, Command, Call, InternalCommand, to_runtime_error};
use crate::errors::JobError;

#[derive(Clone)]
pub struct Echo {}

impl InternalCommand for Echo {
    fn run(
        &mut self,
        _input_type: &Vec<CellType>,
        arguments: &Vec<Argument>,
        _input: &mut dyn InputStream,
        output: &mut dyn OutputStream) -> Result<(), JobError> {
        let g = arguments.iter().map(|c| c.cell.clone());
        output.add(Row {
            cells: g.collect()
        });
        return Ok(());
    }
}

impl Command for Echo {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, JobError> {
        let output_type = arguments
            .iter()
            .map(|a| CellType { name: a.name.clone(), cell_type: a.cell.cell_data_type() })
            .collect();
        return Ok(Box::new(InternalCall {
            name: String::from("echo"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type,
            command: Box::new(self.clone()),
        }));
    }
}
