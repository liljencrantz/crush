use std::path::PathBuf;
use crate::{argument_error_legacy, CrushResult, Printer};
use crate::data::r#struct::Struct;
use crate::lang::argument::Argument;
use crate::lang::command::Command;
use crate::lang::errors::error;
use crate::lang::value::{Value, ValueType};
use crate::util::glob::Glob;
use crate::util::replace::Replace;

macro_rules! argument_getter {
    ($name:ident, $return_type:ty, $value_type:ident, $description:literal) => {
        fn $name(&mut self, idx: usize) -> CrushResult<$return_type> {
            if idx < self.len() {
                let l = self[idx].location;
                match self
                    .replace(idx, Argument::unnamed(Value::Bool(false), l))
                    .value
                {
                    Value::$value_type(s) => Ok(s),
                    v => argument_error_legacy(
                        format!(
                            concat!("Invalid value, expected a ", $description, ", found a {}"),
                            v.value_type().to_string()
                        ),
                    ),
                }
            } else {
                error("Index out of bounds")
            }
        }
    };
}

macro_rules! optional_argument_getter {
    ($name:ident, $return_type:ty, $method:ident) => {
        fn $name(&mut self, idx: usize) -> CrushResult<Option<$return_type>> {
            match self.len() - idx {
                0 => Ok(None),
                1 => Ok(Some(self.$method(idx)?)),
                _ => argument_error_legacy("Wrong number of arguments"),
            }
        }
    };
}

pub trait ArgumentVector {
    fn check_len(&self, len: usize) -> CrushResult<()>;
    fn check_len_range(&self, min_len: usize, max_len: usize) -> CrushResult<()>;
    fn check_len_min(&self, min_len: usize) -> CrushResult<()>;
    fn string(&mut self, idx: usize) -> CrushResult<String>;
    fn integer(&mut self, idx: usize) -> CrushResult<i128>;
    fn float(&mut self, idx: usize) -> CrushResult<f64>;
    fn file(&mut self, idx: usize) -> CrushResult<PathBuf>;
    fn command(&mut self, idx: usize) -> CrushResult<Command>;
    fn r#type(&mut self, idx: usize) -> CrushResult<ValueType>;
    fn value(&mut self, idx: usize) -> CrushResult<Value>;
    fn glob(&mut self, idx: usize) -> CrushResult<Glob>;
    fn r#struct(&mut self, idx: usize) -> CrushResult<Struct>;
    fn bool(&mut self, idx: usize) -> CrushResult<bool>;
    fn files(&mut self, printer: &Printer) -> CrushResult<Vec<PathBuf>>;
    fn optional_bool(&mut self, idx: usize) -> CrushResult<Option<bool>>;
    fn optional_integer(&mut self, idx: usize) -> CrushResult<Option<i128>>;
    fn optional_string(&mut self, idx: usize) -> CrushResult<Option<String>>;
    fn optional_command(&mut self, idx: usize) -> CrushResult<Option<Command>>;
    fn optional_value(&mut self, idx: usize) -> CrushResult<Option<Value>>;
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

    fn check_len_range(&self, min_len: usize, max_len: usize) -> CrushResult<()> {
        if self.len() < min_len {
            argument_error_legacy(
                format!(
                    "Expected at least {} arguments, got {}",
                    min_len,
                    self.len()
                )
                    .as_str(),
            )
        } else if self.len() > max_len {
            argument_error_legacy(
                format!("Expected at most {} arguments, got {}", max_len, self.len()).as_str(),
            )
        } else {
            Ok(())
        }
    }

    fn check_len_min(&self, min_len: usize) -> CrushResult<()> {
        if self.len() >= min_len {
            Ok(())
        } else {
            argument_error_legacy(
                format!(
                    "Expected at least {} arguments, got {}",
                    min_len,
                    self.len()
                )
                    .as_str(),
            )
        }
    }

    fn string(&mut self, idx: usize) -> CrushResult<String> {
        if idx < self.len() {
            let l = self[idx].location;
            match self
                .replace(idx, Argument::unnamed(Value::Empty, l))
                .value
            {
                Value::String(s) => Ok(s.to_string()),
                v => argument_error_legacy(
                    format!(
                        concat!("Invalid value, expected a string found a {}"),
                        v.value_type().to_string()
                    ),
                ),
            }
        } else {
            error("Index out of bounds")
        }
    }

    fn file(&mut self, idx: usize) -> CrushResult<PathBuf> {
        if idx < self.len() {
            let l = self[idx].location;
            match self
                .replace(idx, Argument::unnamed(Value::Empty, l))
                .value
            {
                Value::File(s) => Ok(s.to_path_buf()),
                v => argument_error_legacy(
                    format!(
                        concat!("Invalid value, expected a file found a {}"),
                        v.value_type().to_string()
                    ),
                ),
            }
        } else {
            error("Index out of bounds")
        }
    }

    argument_getter!(integer, i128, Integer, "integer");
    argument_getter!(float, f64, Float, "float");
    argument_getter!(command, Command, Command, "command");
    argument_getter!(r#type, ValueType, Type, "type");
    argument_getter!(glob, Glob, Glob, "glob");
    argument_getter!(r#struct, Struct, Struct, "struct");
    argument_getter!(bool, bool, Bool, "bool");

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

    fn files(&mut self, printer: &Printer) -> CrushResult<Vec<PathBuf>> {
        let mut files = Vec::new();
        for a in self.drain(..) {
            a.value.file_expand(&mut files, printer)?;
        }
        Ok(files)
    }

    optional_argument_getter!(optional_bool, bool, bool);
    optional_argument_getter!(optional_integer, i128, integer);
    optional_argument_getter!(optional_string, String, string);
    optional_argument_getter!(optional_command, Command, command);
    optional_argument_getter!(optional_value, Value, value);
}
