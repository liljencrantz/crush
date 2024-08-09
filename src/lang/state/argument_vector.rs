use crate::{argument_error_legacy, CrushResult};
use crate::lang::argument::Argument;
use crate::lang::errors::error;
use crate::lang::value::{Value};
use crate::util::replace::Replace;

pub trait ArgumentVector {
    fn check_len(&self, len: usize) -> CrushResult<()>;
    fn value(&mut self, idx: usize) -> CrushResult<Value>;
}

impl ArgumentVector for Vec<Argument> {
    fn check_len(&self, len: usize) -> CrushResult<()> {
        if self.len() == len {
            Ok(())
        } else {
            argument_error_legacy(
                format!("Expected {} arguments, got {}", len, self.len()).as_str(),
            )
        }
    }

    fn value(&mut self, idx: usize) -> CrushResult<Value> {
        if idx < self.len() {
            let l = self[idx].location;
            Ok(self
                .replace(idx, Argument::unnamed(Value::Bool(false), l))
                .value)
        } else {
            error("Index out of bounds")
        }
    }
}
