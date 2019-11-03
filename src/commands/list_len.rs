use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error};
use crate::data::CellType;
use crate::data::Row;
use crate::data::Cell;
use crate::data::ColumnType;
use crate::env::get_cwd;

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(
        vec![ColumnType::named("lenght", CellType::Integer)])?;
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].cell) {
        (None, Cell::List(l)) => output.send(Row { cells: vec![Cell::Integer(l.len() as i128)] }),
        _ => Err(argument_error("Argument is not a list")),
    }
}
