mod cell;
mod cell_definition;
mod cell_type;
mod row;
mod rows;
mod argument;
mod call_definition;
mod column_type;
mod list;
mod dict;
mod list_definition;
mod cell_type_lexer;
mod cell_type_parser;

use crate::commands::{CompileContext};
use crate::errors::{JobResult, error};
use std::fmt::Formatter;
use crate::stream::{InputStream};

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
pub use dict::Dict;
pub use list_definition::ListDefinition;
pub use argument::ArgumentVecCompiler;


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
pub struct Output {
    pub stream: InputStream,
}

impl Output {
    pub fn get(&self, idx: i128) -> JobResult<Row> {
        let mut i = 0i128;
        loop {
            match self.stream.recv() {
                Ok(row) => {
                    if i == idx {
                        return Ok(row);
                    }
                    i += 1;
                },
                Err(_) => return Err(error("Index out of bounds")),
            }
        }
    }
}
