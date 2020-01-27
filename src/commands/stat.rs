use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error, to_job_error};
use crate::data::{Argument, Struct};
use crate::data::Value;
use std::fs::metadata;
use std::path::Path;
use crate::stream::ValueSender;
use std::os::unix::fs::MetadataExt;

fn parse(arguments: Vec<Argument>) -> JobResult<Box<Path>> {
    let mut files: Vec<Box<Path>> = Vec::new();
    for arg in &arguments {
        arg.value.file_expand(&mut files)?;
    }
    if files.len() != 1 {
        return Err(argument_error("Expected exactly one file"));
    }
    Ok(files.remove(0))
}

fn run(file: Box<Path>, sender: ValueSender) -> JobResult<()> {
    let metadata = to_job_error(metadata(file))?;
    sender.send(
        Value::Struct(
            Struct::new(
                vec![
                    ("is_directory", Value::Bool(metadata.is_dir())),
                    ("is_file", Value::Bool(metadata.is_file())),
                    ("is_symlink", Value::Bool(metadata.file_type().is_symlink())),
                    ("inode", Value::Integer(metadata.ino() as i128)),
                    ("nlink", Value::Integer(metadata.nlink() as i128)),
                    ("mode", Value::Integer(metadata.mode() as i128)),
                    ("len", Value::Integer(metadata.len() as i128)),
                ]
            )
        )
    )
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    run(parse(context.arguments)?, context.output)
}
