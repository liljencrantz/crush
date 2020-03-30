use crate::lang::execution_context::{ExecutionContext, This, ArgumentVector};
use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::r#struct::Struct;
use crate::lang::value::Value;
use std::fs::metadata;
use std::os::unix::fs::MetadataExt;
use lazy_static::lazy_static;
use std::collections::HashMap;
use crate::lang::command::CrushCommand;

lazy_static! {
    pub static ref METHODS: HashMap<Box<str>, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<Box<str>, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.insert(Box::from("stat"), CrushCommand::command(
            stat, true,
            "file:stat",
            "Return a struct with information about a file.",
            Some(r#"    The return value contains the following fields:

    * is_directory:bool is the file is a directory
    * is_file:bool is the file a regular file
    * is_symlink:bool is the file a symbolic link
    * inode:integer the inode number of the file
    * nlink:integer the number of hardlinks to the file
    * mode:integer the permission bits for the file
    * len: integer the size of the file"#)));

        res.insert(Box::from("exists"), CrushCommand::command(
            exists, true,
            "file:exists",
            "Return true if this file exists",
            None));
        res.insert(Box::from("__getitem__"), CrushCommand::command(
            getitem, true,
            "file[name:string]",
            "Return a file or subdirectory in the specified base directory",
            None));
        res
    };
}

pub fn stat(context: ExecutionContext) -> CrushResult<()> {
    let file = context.this.file()?;
    let metadata = to_crush_error(metadata(file))?;
    context.output.send(
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
                ],
                None,
            )
        )
    )
}

pub fn exists(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::Bool(context.this.file()?.exists()))
}

pub fn getitem(mut context: ExecutionContext) -> CrushResult<()> {
    let base_directory = context.this.file()?;
    context.arguments.check_len(1)?;
    let sub = context.arguments.string(0)?;
    context.output.send(Value::File(base_directory.join(sub.as_ref()).into_boxed_path()))
}
