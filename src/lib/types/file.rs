use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::execution_context::{CommandContext, This};
use crate::lang::data::r#struct::Struct;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use std::fs::metadata;
use std::os::unix::fs::MetadataExt;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "file"];
        Stat::declare_method(&mut res, &path);
//        Chmod::declare_method(&mut res, &path);
        Exists::declare_method(&mut res, &path);
        GetItem::declare_method(&mut res, &path);
        res
    };
}

#[signature(
stat,
can_block = false,
output = Known(ValueType::Struct),
short = "Return a struct with information about a file.",
long = "The return value contains the following fields:",
long = "* is_directory:bool is the file is a directory",
long = "* is_file:bool is the file a regular file",
long = "* is_symlink:bool is the file a symbolic link",
long = "* inode:integer the inode number of the file",
long = "* nlink:integer the number of hardlinks to the file",
long = "* mode:integer the permission bits for the file",
long = "* len: integer the size of the file"
)]
struct Stat {
}

pub fn stat(context: CommandContext) -> CrushResult<()> {
    let file = context.this.file()?;
    let metadata = to_crush_error(metadata(file))?;
    context.output.send(Value::Struct(Struct::new(
        vec![
            ("is_directory".to_string(), Value::Bool(metadata.is_dir())),
            ("is_file".to_string(), Value::Bool(metadata.is_file())),
            (
                "is_symlink".to_string(),
                Value::Bool(metadata.file_type().is_symlink()),
            ),
            ("inode".to_string(), Value::Integer(metadata.ino() as i128)),
            (
                "nlink".to_string(),
                Value::Integer(metadata.nlink() as i128),
            ),
            ("mode".to_string(), Value::Integer(metadata.mode() as i128)),
            ("len".to_string(), Value::Integer(metadata.len() as i128)),
        ],
        None,
    )))
}

#[signature(
chmod,
can_block = false,
output = Known(ValueType::Empty),
short = "Change permissions of this file.",
)]
struct Chmod {
    #[unnamed()]
    permissions: Vec<String>,
}

pub fn chmod(context: CommandContext) -> CrushResult<()> {
    let cfg: Chmod = Chmod::parse(context.arguments, &context.printer)?;

    context
        .output
        .send(Value::Bool(context.this.file()?.exists()))
}

#[signature(
exists,
can_block = false,
output = Known(ValueType::Bool),
short = "True if the file exists.",
)]
struct Exists {}

pub fn exists(context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::Bool(context.this.file()?.exists()))
}

#[signature(
__getitem__,
can_block = false,
output = Known(ValueType::Bool),
short = "Return a file or subdirectory in the specified base directory.",
)]
struct GetItem {
    name: String,
}
pub fn __getitem__(context: CommandContext) -> CrushResult<()> {
    let base_directory = context.this.file()?;
    let cfg: GetItem = GetItem::parse(context.arguments, &context.printer)?;
    context.output.send(Value::File(base_directory.join(&cfg.name)))
}
