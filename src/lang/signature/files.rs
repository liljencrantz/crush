use crate::lang::argument::ValidationArgument;
use crate::lang::command::{OutputType, PipeInputType};
use crate::lang::data::binary::{BinaryWriter, binary_channel};
use crate::lang::errors::{CrushError, CrushResult, command_error};
use crate::lang::job_control::ChannelBasedController;
use crate::lang::pipe::ValueSender;
use crate::lang::state::handles::CommandHandle;
use crate::lang::value::{Value, ValueType};
use crate::util::file::cwd;
use crate::util::glob::Glob;
use crate::util::regex::RegexFileMatcher;
use crossbeam::channel::unbounded;
use regex::Regex;
use std::convert::TryFrom;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/**
A type representing one or more files.
 */
#[derive(Debug, Clone)]
pub enum Files {
    File(Arc<Path>),
    Glob(Glob),
    Regex(Regex),
    String(Arc<str>),
}

impl TryFrom<Value> for Files {
    type Error = CrushError;

    fn try_from(value: Value) -> CrushResult<Self> {
        match value {
            Value::String(v) => Ok(Files::String(v)),
            Value::File(v) => Ok(Files::File(v)),
            Value::Glob(v) => Ok(Files::Glob(v)),
            Value::Regex(_, v) => Ok(Files::Regex(v)),
            v => command_error(format!(
                "Invalid type `{}`, expected `one_of $file $glob $binary`.",
                v.value_type()
            )),
        }
    }
}

impl TryInto<Vec<PathBuf>> for Files {
    type Error = CrushError;

    fn try_into(self) -> CrushResult<Vec<PathBuf>> {
        match self {
            Files::String(p) => Ok(vec![PathBuf::from(p.as_ref())]),
            Files::File(p) => Ok(vec![p.to_path_buf()]),
            Files::Glob(pattern) => {
                let mut tmp = Vec::new();
                pattern.glob_files(&cwd()?, &mut tmp)?;
                Ok(tmp.into_iter().collect())
            }
            Files::Regex(pattern) => {
                let mut tmp = Vec::new();
                pattern.match_files(&cwd()?, &mut tmp)?;
                Ok(tmp.into_iter().collect())
            }
        }
    }
}

pub fn into_paths(files: Vec<Files>) -> CrushResult<Vec<PathBuf>> {
    let mut res = Vec::new();
    for i in files {
        res.append(&mut <Files as TryInto<Vec<PathBuf>>>::try_into(i)?);
    }
    Ok(res)
}

impl TryInto<Box<dyn BinaryWriter>> for Files {
    type Error = CrushError;

    fn try_into(self) -> Result<Box<dyn BinaryWriter>, Self::Error> {
        let vec: Vec<_> = self.try_into()?;
        match vec.len() {
            0 => command_error("No write target specified."),
            1 => Ok(Box::from(File::create(&vec[0])?)),
            n => command_error(format!(
                "Single write targets expected, found {} different write targets.",
                n
            )),
        }
    }
}

/// Same job-control wiring as `binary_input::register_control`, for the write side --
/// see that function's doc comment. Only a channel-backed writer does anything with it;
/// a plain file has no reader to wait on and just ignores this via `BinaryWriter`'s
/// default no-op.
fn register_control(writer: &mut Box<dyn BinaryWriter>, command_handle: &CommandHandle) {
    let (control_sender, control_receiver) = unbounded();
    command_handle.register(Arc::from(ChannelBasedController::new(control_sender)));
    writer.register_control(control_receiver);
}

pub fn writer(
    files: Option<Files>,
    output: ValueSender,
    command_handle: &CommandHandle,
) -> CrushResult<Box<dyn BinaryWriter>> {
    match files {
        None => {
            let (mut w, r) = binary_channel();
            register_control(&mut w, command_handle);
            output.send(Value::BinaryInputStream(r))?;
            Ok(w)
        }
        Some(file) => {
            output.send(Value::Empty)?;
            Ok(file.try_into()?)
        }
    }
}

/// Computes the real output type of any `*:to` command built on `writer` above: whether
/// a `file` argument was given at all decides it, matching `writer`'s own behavior
/// exactly -- given a destination file, it writes there and sends `Empty`; given none,
/// it sends back a `binary_stream` instead. The argument's own value never matters here,
/// only whether one was supplied, so this works even when that value itself isn't
/// resolved to anything yet. `file` must be each such command's first declared field (as
/// it is for every one of them today) for this to find it correctly by position when
/// it's given unnamed; a later, unrelated field that can also be given positionally
/// (e.g. `$json.to`'s `compact`) is never mistaken for it.
pub fn writer_dynamic_output(
    _input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let mut seen_positional = false;
    let file_given = arguments.iter().any(|arg| match &arg.argument_type {
        Some(name) => name == "file",
        None => {
            let first = !seen_positional;
            seen_positional = true;
            first
        }
    });
    Ok(OutputType::Known(if file_given {
        ValueType::Empty
    } else {
        ValueType::BinaryInputStream
    }))
}

pub fn path(files: Option<Files>, fallback: impl Into<PathBuf>) -> CrushResult<PathBuf> {
    match files {
        None => Ok(fallback.into()),
        Some(file) => {
            let mut dir: Vec<PathBuf> = file.try_into()?;
            match dir.len() {
                0 => command_error("No path specified."),
                1 => Ok(dir.pop().unwrap()),
                n => command_error(format!(
                    "Single path expected, found {} different paths.",
                    n
                )),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::{ArgumentValue, SwitchStyle};
    use crate::lang::ast::source::{Source, SourceType};
    use std::sync::Arc;

    fn unnamed_arg(value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: None,
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    fn named_arg(name: &str, value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: Some(name.to_string()),
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    #[test]
    fn no_file_argument_is_binary_input_stream() {
        let out = writer_dynamic_output(PipeInputType::None, &[]).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::BinaryInputStream)));
    }

    #[test]
    fn an_unnamed_file_argument_is_empty() {
        let args = [unnamed_arg(ArgumentValue::Value(Value::from("out.bin")))];
        let out = writer_dynamic_output(PipeInputType::None, &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Empty)));
    }

    #[test]
    fn a_named_file_argument_is_empty() {
        let args = [named_arg("file", ArgumentValue::Value(Value::from("out.bin")))];
        let out = writer_dynamic_output(PipeInputType::None, &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Empty)));
    }

    #[test]
    fn an_unresolved_file_argument_is_still_empty() {
        let args = [unnamed_arg(ArgumentValue::Unknown)];
        let out = writer_dynamic_output(PipeInputType::None, &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Empty)));
    }

    /// A second, unrelated unnamed argument (like $json.to's `compact`, given
    /// positionally) must never cause this to look past it and reconsider -- only the
    /// *first* unnamed argument is ever treated as `file`.
    #[test]
    fn a_second_unnamed_argument_does_not_change_the_result() {
        let args = [
            unnamed_arg(ArgumentValue::Value(Value::from("out.bin"))),
            unnamed_arg(ArgumentValue::Value(Value::Bool(true))),
        ];
        let out = writer_dynamic_output(PipeInputType::None, &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Empty)));
    }
}
