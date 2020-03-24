use crate::lang::{argument::Argument, value::Value};
use crate::lang::errors::{CrushResult, argument_error};
use std::path::Path;
use crate::lang::command::CrushCommand;

pub fn argument_files(mut arguments: Vec<Argument>) -> CrushResult<Vec<Box<Path>>> {
    let mut files = Vec::new();
    for a in arguments.drain(..) {
        a.value.file_expand(&mut files)?;
    }
    Ok(files)
}

pub fn optional_argument_integer(mut arg: Vec<Argument>) -> CrushResult<Option<i128>> {
    match arg.len() {
        0 => Ok(None),
        1 => {
            let a = arg.remove(0);
            match (a.name, a.value) {
                (None, Value::Integer(i)) => Ok(Some(i)),
                _ => argument_error("Expected a text value"),
            }
        }
        _ => argument_error("Expected a single value"),
    }
}
