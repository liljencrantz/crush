use crate::lang::command::ExecutionContext;
use crate::errors::{CrushResult, argument_error, to_crush_error};
use crate::lang::{argument::Argument, r#struct::Struct};
use crate::lang::value::Value;
use std::fs::metadata;
use std::path::Path;
use crate::lang::stream::ValueSender;
use std::os::unix::fs::MetadataExt;

fn parse(arguments: Vec<Argument>) -> CrushResult<Box<Path>> {
    let mut files: Vec<Box<Path>> = Vec::new();
    for arg in &arguments {
        arg.value.file_expand(&mut files)?;
    }
    if files.len() != 1 {
        return argument_error("Expected exactly one file");
    }
    Ok(files.remove(0))
}

fn run(file: Box<Path>, sender: ValueSender) -> CrushResult<()> {
    let metadata = to_crush_error(metadata(file))?;
    sender.send(
        Value::Struct(
            Struct::new(
                vec![
                    (Box::from("is_directory"), Value::Bool(metadata.is_dir())),
                    (Box::from("is_file"), Value::Bool(metadata.is_file())),
                    (Box::from("is_symlink"), Value::Bool(metadata.file_type().is_symlink())),
                    (Box::from("inode"), Value::Integer(metadata.ino() as i128)),
                    (Box::from("nlink"), Value::Integer(metadata.nlink() as i128)),
                    (Box::from("mode"), Value::Integer(metadata.mode() as i128)),
                    (Box::from("len"), Value::Integer(metadata.len() as i128)),
                ]
            )
        )
    )
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    run(parse(context.arguments)?, context.output)
}
