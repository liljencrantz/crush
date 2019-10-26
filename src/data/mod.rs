mod cell;
mod cell_definition;
mod cell_type;
mod row;
mod rows;
mod argument;
mod call_definition;
mod column_type;
mod list;
mod list_definition;

use crate::commands::{Exec, CompileContext};
use crate::errors::{JobError, error, JobResult};
use std::fmt::Formatter;
use crate::stream::{InputStream, OutputStream, UninitializedInputStream};
use std::hash::Hasher;
use regex::Regex;
use std::error::Error;

pub use cell::Cell;
pub use column_type::ColumnType;
pub use cell_type::CellType;
pub use cell_definition::CellDefinition;
pub use cell::Alignment;
pub use argument::Argument;
pub use argument::BaseArgument;
pub use argument::ArgumentDefinition;
pub use row::Row;
pub use rows::Rows;
pub use call_definition::CallDefinition;
pub use list::List;
pub use list_definition::ListDefinition;

use crate::glob::Glob;


#[derive(Clone)]
pub struct Command {
    pub call: fn(context: CompileContext) -> JobResult<()>,
}

impl Command {
    pub fn new(call: fn(context: CompileContext) -> JobResult<()>) -> Command {
        return Command { call };
    }
}

impl std::cmp::PartialEq for Command {
    fn eq(&self, _other: &Command) -> bool {
        return false;
    }
}

impl std::cmp::Eq for Command {}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command")
    }
}

#[derive(Debug)]
pub struct JobOutput {
    pub stream: InputStream,
}
