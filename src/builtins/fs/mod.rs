use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::file::{home};
use signature::signature;
use crate::lang::signature::files::Files;
use std::path::PathBuf;
use std::convert::TryFrom;
use std::sync::Arc;
use chrono::{DateTime, Local};
use libc::{S_IFBLK, S_IFCHR, S_IFDIR, S_IFIFO, S_IFLNK, S_IFREG, S_IFSOCK};
use nix::sys::stat::lstat;
use crate::lang::data::table::{ColumnType, Row};

mod usage;
mod files;
mod mounts;
pub mod fd;

#[signature(
    fs.cd,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Change to the specified working directory.",
)]
struct Cd {
    #[unnamed()]
    #[description("the new working directory.")]
    destination: Files,
}

fn cd(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Cd = Cd::parse(context.remove_arguments(), &context.global_state.printer())?;

    let dir = match cfg.destination.had_entries() {
        true => PathBuf::try_from(cfg.destination),
        false => home(),
    }?;

    std::env::set_current_dir(dir)?;
    context.output.send(Value::Empty)
}

static STAT_OUTPUT_TYPE: [ColumnType; 18] = [
    ColumnType::new("is_socket", ValueType::Bool),
    ColumnType::new("is_symlink", ValueType::Bool),
    ColumnType::new("is_file", ValueType::Bool),
    ColumnType::new("is_block_device", ValueType::Bool),
    ColumnType::new("is_directory", ValueType::Bool),
    ColumnType::new("is_character_device", ValueType::Bool),
    ColumnType::new("is_fifo", ValueType::Float),
    ColumnType::new("inode", ValueType::Integer),
    ColumnType::new("nlink", ValueType::Integer),
    ColumnType::new("uid", ValueType::Integer),
    ColumnType::new("gid", ValueType::Integer),
    ColumnType::new("size", ValueType::Integer),
    ColumnType::new("block_size", ValueType::Integer),
    ColumnType::new("blocks", ValueType::Integer),
    ColumnType::new("access_time", ValueType::Time),
    ColumnType::new("modification_time", ValueType::Time),
    ColumnType::new("creation_time", ValueType::Time),
    ColumnType::new("file", ValueType::File),
];

#[signature(
    fs.stat,
    can_block = true,
    output = Known(ValueType::table_input_stream(&STAT_OUTPUT_TYPE)),
    short = "Return a row with information about each file",
    long = "The return value contains the following columns:",
    long = "* is_socket:bool is the file is a socket",
    long = "* is_symlink:bool is the file a symbolic link",
    long = "* is_block_device:bool is the file a block device",
    long = "* is_directory:bool is the file is a directory",
    long = "* is_character_device:bool is the file a character_device",
    long = "* is_fifo:bool is the file a fifo",
    long = "* inode:integer the inode number of the file",
    long = "* nlink:integer the number of hardlinks to the file",
    long = "* uid:integer The user id of the file owner",
    long = "* gid:integer The group id of the file owner",
    long = "* size:integer File size in bytes",
    long = "* block_size:integer The size of a single block on the device storing this file",
    long = "* blocks:integer The number of blocks used to store this file",
    long = "* access_time:time The last time this file was accessed",
    long = "* modification_time:time The last time this file was modified",
    long = "* creation_time:time The time this file was created",
    long = "* file:path The filename",
)]
struct Stat {
    #[unnamed()]
    #[description("the files to show the status for.")]
    destination: Files,
    #[description("stat symlinks, not the files they point to.")]
    #[default(false)]
    symlink: bool,
}

fn stat(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Stat = Stat::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.initialize(&STAT_OUTPUT_TYPE)?;

    let v : Vec<PathBuf> = cfg.destination.into();
    for file in v {

        let metadata =
            if cfg.symlink {
                lstat(&file)
            } else {
                nix::sys::stat::stat(&file)
            }?;

        output.send(Row::new(vec![
            Value::Bool((metadata.st_mode & S_IFSOCK) != 0),
            Value::Bool((metadata.st_mode & S_IFLNK) != 0),
            Value::Bool((metadata.st_mode & S_IFREG) != 0),
            Value::Bool((metadata.st_mode & S_IFBLK) != 0),
            Value::Bool((metadata.st_mode & S_IFDIR) != 0),
            Value::Bool((metadata.st_mode & S_IFCHR) != 0),
            Value::Bool((metadata.st_mode & S_IFIFO) != 0),
            Value::Integer(metadata.st_ino as i128),
            Value::Integer(metadata.st_nlink as i128),
            Value::Integer(metadata.st_uid as i128),
            Value::Integer(metadata.st_gid as i128),
            Value::Integer(metadata.st_size as i128),
            Value::Integer(metadata.st_blksize as i128),
            Value::Integer(metadata.st_blocks as i128),
            Value::Time(DateTime::from_timestamp(metadata.st_atime, 0).unwrap().with_timezone(&Local)),
            Value::Time(DateTime::from_timestamp(metadata.st_mtime, 0).unwrap().with_timezone(&Local)),
            Value::Time(DateTime::from_timestamp(metadata.st_ctime, 0).unwrap().with_timezone(&Local)),
            Value::File(Arc::from(file.as_path())),
        ]))?;
    }
    context.output.send(Value::Empty)
}

#[signature(
    fs.cwd,
    can_block = false,
    output = Known(ValueType::File),
    short = "Return the current working directory.",
)]
struct Cwd {}

fn cwd(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::from(crate::util::file::cwd()?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "fs",
        "File system functionality",
        Box::new(move |fs| {
            files::FilesSignature::declare(fs)?;
            Cd::declare(fs)?;
            mounts::Mounts::declare(fs)?;
            Cwd::declare(fs)?;
            Stat::declare(fs)?;
            usage::Usage::declare(fs)?;
            fs.create_namespace(
                "fd",
                "Information on currently open files and sockets",
                Box::new(move |fd| {
                    fd::File::declare(fd)?;
                    #[cfg(target_os = "linux")]
                    fd::procfs::Network::declare(fd)?;
                    #[cfg(target_os = "linux")]
                    fd::procfs::Unix::declare(fd)?;
                    Ok(())
                }),
            )?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
