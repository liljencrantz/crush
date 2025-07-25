use crate::lang::argument::Argument;
use crate::lang::errors::error;
use crate::lang::value::Value;
use crate::util::replace::Replace;
use crate::{CrushResult, command_error};

pub trait ArgumentVector {
    fn check_len(&self, len: usize) -> CrushResult<()>;
    fn value(&mut self, idx: usize) -> CrushResult<Value>;
}

impl ArgumentVector for Vec<Argument> {
    fn check_len(&self, len: usize) -> CrushResult<()> {
        if self.len() == len {
            Ok(())
        } else {
            command_error(format!("Expected {} arguments, got {}", len, self.len()).as_str())
        }
    }

    fn value(&mut self, idx: usize) -> CrushResult<Value> {
        if idx < self.len() {
            let source = self[idx].source.clone();
            Ok(self
                .replace(idx, Argument::unnamed(Value::Bool(false), &source))
                .value)
        } else {
            error("Index out of bounds")
        }
    }
}
