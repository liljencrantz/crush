use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::{
    lang::{
        Row,
        ValueType,
        Value
    }
};
use crate::lang::{ColumnType, Argument, TableReader};
use crate::lib::command_util::find_field_from_str;
use crate::stream::{Readable};

pub fn parse(input_type: &Vec<ColumnType>, arguments: &Vec<Argument>) -> CrushResult<usize> {
    match arguments.len() {
        0 => if input_type.len() == 1 && input_type[0].cell_type == ValueType::Integer{
            Ok(0)
        } else {
            error("Unexpected input format, expected a single column of integers")
        },
        1 => {
            if let Value::Field(f) = &arguments[0].value {
                match f.len() {
                    1 => {
                        Ok(find_field_from_str(f[0].as_ref(), input_type)?)
                    }
                    _ => {
                        error("Path contains too many elements")
                    }
                }
            } else {
                error("Unexpected cell type, expected field")
            }
        },
        _ => error("Expected exactly one argument, a field defintition")
    }
}

fn sum_rows(mut s: Box<dyn Readable>, column: usize) -> CrushResult<Value> {
    let mut res: i128 = 0;
    loop {
        match s.read() {
            Ok(row) => match row.cells()[column] {
                Value::Integer(i) => res += i,
                _ => return error("Invalid cell value, expected an integer")
            },
            Err(_) => break,
        }
    }
    Ok(Value::Integer(res))
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(input) => {
            let column = parse(input.types(), &context.arguments)?;
            context.output.send(sum_rows(input, column)?)
        }
        _ => error("Expected a stream"),
    }
}
