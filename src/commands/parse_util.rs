use crate::data::{Argument, Value};
use crate::errors::{CrushResult, argument_error};

pub fn single_argument_text(mut arg: Vec<Argument>) -> CrushResult<Box<str>> {
    match arg.len() {
        1 => {
            match arg.remove(0).value {
                Value::Text(t) => Ok(t),
                _ => Err(argument_error("Expected a text value")),
            }
        }
        _ => Err(argument_error("Expected a single value")),
    }
}
