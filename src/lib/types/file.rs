use crate::lang::command::{ExecutionContext, This, CrushCommand};
use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::r#struct::Struct;
use crate::lang::value::Value;
use std::fs::metadata;
use std::path::Path;
use crate::lang::stream::ValueSender;
use std::os::unix::fs::MetadataExt;
use lazy_static::lazy_static;
use std::collections::HashMap;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand + Send + Sync>> = HashMap::new();
        res.insert(Box::from("stat"), CrushCommand::command(stat, true));
        res
    };
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

pub fn stat(context: ExecutionContext) -> CrushResult<()> {
    run(context.this.file()?, context.output)
}
