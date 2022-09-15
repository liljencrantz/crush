use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{to_crush_error, CrushResult, argument_error_legacy, mandate};
use crate::lang::state::contexts::{CommandContext, This};
use crate::lang::data::r#struct::Struct;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use std::fs::{File, metadata};
use std::os::unix::fs::MetadataExt;
use signature::signature;
use std::collections::HashSet;
use crate::lib::types::file::PermissionAdjustment::{Add, Remove, Set};
use std::os::unix::fs::PermissionsExt;
use crate::data::binary::BinaryReader;
use crate::util::user_map::{get_uid, get_gid};

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "file"];
        Stat::declare_method(&mut res, &path);
        Chown::declare_method(&mut res, &path);
        Chmod::declare_method(&mut res, &path);
        Exists::declare_method(&mut res, &path);
        GetItem::declare_method(&mut res, &path);
        Write::declare_method(&mut res, &path);
        Read::declare_method(&mut res, &path);
        Parent::declare_method(&mut res, &path);
        Name::declare_method(&mut res, &path);
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
struct Stat {}

pub fn stat(mut context: CommandContext) -> CrushResult<()> {
    let file = context.this.file()?;
    let metadata = to_crush_error(metadata(file))?;
    context.output.send(Value::Struct(Struct::new(
        vec![
            ("is_directory", Value::Bool(metadata.is_dir())),
            ("is_file", Value::Bool(metadata.is_file())),
            ("is_symlink", Value::Bool(metadata.file_type().is_symlink())),
            ("inode", Value::Integer(metadata.ino() as i128)),
            ("nlink", Value::Integer(metadata.nlink() as i128)),
            ("mode", Value::Integer(metadata.mode() as i128)),
            ("len", Value::Integer(metadata.len() as i128)),
        ],
        None,
    )))
}

#[signature(
chown,
can_block = false,
output = Known(ValueType::Empty),
short = "Change owner of this file.",
)]
struct Chown {
    #[description("the owning user for the file.")]
    user: Option<String>,
    #[description("the owning group for the file.")]
    group: Option<String>,
}

pub fn chown(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Chown = Chown::parse(context.arguments, &context.global_state.printer())?;
    let file = context.this.file()?;

    let uid = if let Some(name) = cfg.user {
        Some(mandate(get_uid(&name)?, format!("Unknown user {}", &name))?)
    } else {
        None
    };

    let gid = if let Some(name) = cfg.group {
        Some(mandate(get_gid(&name)?, format!("Unknown group {}", &name))?)
    } else {
        None
    };

    to_crush_error(nix::unistd::chown(&file, uid, gid))?;

    context
        .output
        .send(Value::Empty())
}

#[signature(
chmod,
can_block = false,
output = Known(ValueType::Empty),
short = "Change permissions of this file.",
long = "Permissions are strings of the form [classes...][adjustment][modes..].",
long = "* A class is one of u, g, o, a, signifying file owner, file group, other users and all users, respectively.",
long = "* The adjustment must be one of +, -, and =, signifying added permissions, removed permissions and set permissions, respectively.",
long = "* A mode is one of r w, x, signifying read, write and execute permissions.",
example = "./foo:chmod \"a=\" \"u+r\" # First strip all rights for all users, then re-add read rights for the owner",
)]
struct Chmod {
    #[description("the set of permissions to add.")]
    #[unnamed()]
    permissions: Vec<String>,
}

const OWNER: u32 = 6;
const GROUP: u32 = 3;
const OTHER: u32 = 0;

enum PermissionAdjustment {
    Add,
    Remove,
    Set,
}

const READ: u32 = 4;
const WRITE: u32 = 2;
const EXECUTE: u32 = 1;


fn apply(perm: &str, mut current: u32) -> CrushResult<u32> {
    let mut class_done = false;
    let mut classes = HashSet::new();
    let mut adjustments = PermissionAdjustment::Add;
    let mut modes = 0u32;

    for c in perm.chars() {
        match class_done {
            false => {
                match c {
                    'u' => { classes.insert(OWNER); }
                    'g' => { classes.insert(GROUP); }
                    'o' => { classes.insert(OTHER); }
                    'a' => {
                        classes.insert(OWNER);
                        classes.insert(GROUP);
                        classes.insert(OTHER);
                    }
                    '+' => {
                        class_done = true;
                    }
                    '-' => {
                        adjustments = Remove;
                        class_done = true;
                    }
                    '=' => {
                        adjustments = Set;
                        class_done = true;
                    }
                    c => {
                        return argument_error_legacy(format!("Illegal character in class-part of permission: {}", c));
                    }
                }
            }
            true => {
                match c {
                    'r' => modes |= READ,
                    'w' => modes |= WRITE,
                    'x' => modes |= EXECUTE,
                    c => {
                        return argument_error_legacy(format!("Illegal character in mode-part of permission: {}", c));
                    }
                }
            }
        }
    }

    if !class_done {
        return argument_error_legacy("Premature end of permission");
    }

    if classes.is_empty() {
        return argument_error_legacy("No user classes specified in permission");
    }

    for cl in classes {
        match adjustments {
            Add => {
                // Add new bits
                current |= modes << cl;
            }
            Remove => {
                // Remove bits
                current = current & !(modes << cl);
            }
            Set => {
                // Clear current bits
                current = current & !(7 << cl);
                // Add new bits
                current |= modes << cl;
            }
        }
    }

    Ok(current)
}

pub fn chmod(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Chmod = Chmod::parse(context.arguments, &context.global_state.printer())?;
    let file = context.this.file()?;
    let metadata = to_crush_error(metadata(&file))?;

    let mut current: u32 = metadata.permissions().mode();

    for perm in cfg.permissions {
        current = apply(&perm, current)?;
    }

    to_crush_error(std::fs::set_permissions(&file, std::fs::Permissions::from_mode(current)))?;
    context
        .output
        .send(Value::Empty())
}

#[signature(
exists,
can_block = false,
output = Known(ValueType::Bool),
short = "True if the file exists.",
)]
struct Exists {}

pub fn exists(mut context: CommandContext) -> CrushResult<()> {
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

pub fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let base_directory = context.this.file()?;
    let cfg: GetItem = GetItem::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(Value::File(base_directory.join(&cfg.name)))
}


#[signature(
write,
can_block = true,
output = Known(ValueType::Empty),
short = "A write sink for binary_stream values",
)]
struct Write {}

fn write(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::BinaryInputStream(mut input) => {
            let mut out = to_crush_error(File::create(
                context.this.file()?))?;
            to_crush_error(std::io::copy(input.as_mut(), &mut out))?;
            Ok(())
        }
        _ => argument_error_legacy("Expected a binary stream"),
    }
}

#[signature(
read,
can_block = true,
output = Known(ValueType::BinaryInputStream),
short = "A read source for binary_stream values",
)]
struct Read {}

fn read(mut context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::BinaryInputStream(<dyn BinaryReader>::paths(vec![context.this.file()?])?))
}

#[signature(
name,
can_block = false,
output = Known(ValueType::String),
short = "The name (excluding path) of this file, as a string",
)]
struct Name {}

fn name(mut context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::string(
            mandate(
                mandate(
                    context.this.file()?
                        .file_name(),
                    "Invalid file path")?
                    .to_str(),
                "Invalid file name")?))
}

#[signature(
parent,
can_block = false,
output = Known(ValueType::File),
short = "The parent directory of this file",
)]
struct Parent {}

fn parent(mut context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::File(
                mandate(
                    context.this.file()?.parent(),
                    "Invalid file path")?.to_path_buf()))
}
