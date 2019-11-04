use crate::commands::CompileContext;
use crate::errors::{JobResult, argument_error};
use crate::data::{CellType, Dict};
use crate::data::Row;
use crate::data::Cell;
use crate::data::ColumnType;
use crate::env::get_cwd;

pub fn create(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(vec![])?;
    if context.arguments.len() != 3 {
        return Err(argument_error("Expected 3 arguments to dict.create"));
    }
    match (&context.arguments[0].cell, &context.arguments[1].cell, &context.arguments[2].cell) {
        (Cell::Text(name), Cell::Text(key_type), Cell::Text(value_type)) => {
            context.env.declare(
                name.as_ref(),
                Cell::Dict(Dict::new(CellType::from(key_type)?, CellType::from(value_type)?)));
            Ok(())
        }
        _ => Err(argument_error("Invalid argument types")),
    }
}

pub fn insert(mut context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(vec![])?;
    if context.arguments.len() != 3 {
        return Err(argument_error("Expected three arguments"));
    }
    let value = context.arguments.remove(2).cell;
    let key = context.arguments.remove(1).cell;
    match (&context.arguments[0].cell) {
        (Cell::Dict(dict)) => {
            if dict.key_type() == key.cell_type() && dict.value_type() == value.cell_type() {
                dict.insert(key, value);
                Ok(())
            } else {
                Err(argument_error("Wrong key/value type"))
            }
        }
        _ => Err(argument_error("Argument is not a dict")),
    }
}

pub fn get(mut context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 2 {
        return Err(argument_error("Expected two arguments"));
    }
    match (&context.arguments[0].cell) {
        (Cell::Dict(dict)) => {
            let output = context.output.initialize(
                vec![ColumnType::named("value", dict.value_type())])?;
            dict.get(&context.arguments[1].cell).map(|c| output.send(Row { cells: vec![c] }));
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn remove(mut context: CompileContext) -> JobResult<()> {
    if context.arguments.len() != 2 {
        return Err(argument_error("Expected two arguments"));
    }
    match (&context.arguments[0].cell) {
        (Cell::Dict(dict)) => {
            let output = context.output.initialize(
                vec![ColumnType::named("value", dict.value_type())])?;
            dict.remove(&context.arguments[1].cell).map(|c| output.send(Row { cells: vec![c] }));
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn len(mut context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(
        vec![ColumnType::named("length", CellType::Integer)])?;
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected one argument"));
    }
    match (&context.arguments[0].cell) {
        (Cell::Dict(dict)) => {
            output.send(Row { cells: vec![Cell::Integer(dict.len() as i128)] })?;
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}

pub fn empty(mut context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(
        vec![ColumnType::named("empty", CellType::Bool)])?;
    if context.arguments.len() != 1 {
        return Err(argument_error("Expected one argument"));
    }
    match (&context.arguments[0].cell) {
        (Cell::Dict(dict)) => {
            output.send(Row { cells: vec![Cell::Bool(dict.len() == 0)] })?;
            Ok(())
        }
        _ => Err(argument_error("Argument is not a list")),
    }
}
