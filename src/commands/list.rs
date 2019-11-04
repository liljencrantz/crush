use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error};
use crate::data::CellType;
use crate::data::Row;
use crate::data::Cell;
use crate::data::ColumnType;
use crate::env::get_cwd;

pub fn len_compile_and_run(context: CompileContext) -> JobResult<()> {
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

pub fn empty_compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(
        vec![ColumnType::named("empty", CellType::Bool)])?;
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].cell) {
        (None, Cell::List(l)) => output.send(Row { cells: vec![Cell::Bool(l.len()==0)] }),
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn push_compile_and_run(mut context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(
        vec![ColumnType::named("lenght", CellType::Integer)])?;
    if context.arguments.len() == 0 {
        return Err(argument_error("Expected at least one argument to list.push"));
    }
    let list_cell = context.arguments.remove(0);
    match (&list_cell.name, &list_cell.cell) {
        (None, Cell::List(l)) => {
            let mut new_elements: Vec<Cell> = Vec::new();
            for el in context.arguments.drain(..) {
                if el.cell.cell_type() == l.cell_type() {
                    new_elements.push(el.cell)
                } else {
                    return Err(argument_error("Invalid element type"));
                }
            }
            if !new_elements.is_empty() {
               l.append(&mut new_elements);
            }
            Ok(())
        },
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn pop_compile_and_run(mut context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].cell) {
        (None, Cell::List(l)) => {
            let output = context.output.initialize(
                vec![ColumnType::named("element", l.cell_type())])?;
            l.pop().map(|c| output.send(Row { cells: vec![c] }));
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}
