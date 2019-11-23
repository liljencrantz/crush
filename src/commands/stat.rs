use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error, to_job_error};
use crate::data::{ValueType, Argument, Struct};
use crate::data::Row;
use crate::data::Value;
use crate::data::ColumnType;
use crate::env::get_cwd;
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
    sender.send(Value::Struct(
        Struct {
            types: vec![
                ColumnType::named("is_directory", ValueType::Bool),
                ColumnType::named("is_file", ValueType::Bool),
                ColumnType::named("is_symlink", ValueType::Bool),
                ColumnType::named("inode", ValueType::Integer),
                ColumnType::named("nlink", ValueType::Integer),
                ColumnType::named("mode", ValueType::Integer),
                ColumnType::named("len", ValueType::Integer),
            ],
            cells: vec![
                Value::Bool(metadata.is_dir()),
                Value::Bool(metadata.is_file()),
                Value::Bool(metadata.file_type().is_symlink()),
                Value::Integer(metadata.ino() as i128),
                Value::Integer(metadata.nlink() as i128),
                Value::Integer(metadata.mode() as i128),
                Value::Integer(metadata.len() as i128),
            ],
        }
    ));
    Ok(())
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    run(parse(context.arguments)?, context.output)
}
