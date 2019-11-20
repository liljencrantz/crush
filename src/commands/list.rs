use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error};
use crate::data::{CellType, List};
use crate::data::Row;
use crate::data::Cell;
use crate::data::ColumnType;

pub fn create(context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected 1 argument"));
    }
    match &context.arguments[0].cell {
        Cell::Text(element_type) => {
            context.output.send(Cell::List(List::new(CellType::from(element_type)?, vec![])))
        }
        _ => Err(argument_error("Invalid argument types")),
    }
}

pub fn len(context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].cell) {
        (None, Cell::List(l)) => context.output.send(Cell::Integer(l.len() as i128)),
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn empty(context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].cell) {
        (None, Cell::List(l)) => context.output.send(Cell::Bool(l.len()==0)),
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn push(mut context: CompileContext) -> JobResult<()> {
    if context.arguments.len() == 0 {
        return Err(argument_error("Expected at least one argument to list.push"));
    }
    let list_cell = context.arguments.remove(0);
    match (&list_cell.name, &list_cell.cell) {
        (None, Cell::List(l)) => {
            let mut new_elements: Vec<Cell> = Vec::new();
            for el in context.arguments.drain(..) {
                if el.cell.cell_type() == l.element_type() {
                    new_elements.push(el.cell)
                } else {
                    return Err(argument_error("Invalid element type"));
                }
            }
            if !new_elements.is_empty() {
               l.append(&mut new_elements);
            }
            context.output.send(list_cell.cell);
            Ok(())
        },
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn pop(context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected single argument to list.len"));
    }
    match (&context.arguments[0].name, &context.arguments[0].cell) {
        (None, Cell::List(l)) => {
            l.pop().map(|c| context.output.send(c));
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}
