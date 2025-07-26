use crate::lang::errors::{CrushError, command_error, CrushResult};
use crate::lang::value::{BinaryInputStream, Value};
use std::sync::Arc;

/// A type representing a value with a binary representation. It is used in the signature of builtin commands that
/// accept any type of binary value as arguments.
pub enum BinaryInput {
    BinaryInputStream(BinaryInputStream),
    Binary(Arc<[u8]>),
    /// Will be implicitly converted to bytes using utf-8 encoding.
    String(Arc<str>),
}

impl TryFrom<Value> for BinaryInput {
    type Error = CrushError;

    fn try_from(value: Value) -> CrushResult<Self> {
        match value {
            Value::Binary(v) => Ok(BinaryInput::Binary(v)),
            Value::BinaryInputStream(v) => Ok(BinaryInput::BinaryInputStream(v)),
            Value::String(v) => Ok(BinaryInput::String(v)),
            v => command_error(
                format!(
                    "Invalid type `{}`, expected `one_of $string $binary $binary_input_string`.",
                    v.value_type()
                ),
            ),
        }
    }
}
