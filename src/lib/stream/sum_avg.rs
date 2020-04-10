use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{CrushResult, error, argument_error};
use crate::{
    lang::{
        value::ValueType,
        value::Value,
    }
};
use crate::lang::{table::ColumnType, argument::Argument};
use crate::lang::stream::Readable;
use crate::lang::table::ColumnVec;
use chrono::Duration;

pub fn parse(input_type: &Vec<ColumnType>, arguments: &[Argument]) -> CrushResult<usize> {
    match arguments.len() {
        0 => if input_type.len() == 1 && input_type[0].cell_type == ValueType::Integer {
            Ok(0)
        } else {
            error("Unexpected input format, expected a single column of integers")
        },
        1 => {
            if let Value::Field(f) = &arguments[0].value {
                match f.len() {
                    1 => {
                        Ok(input_type.find_str(f[0].as_ref())?)
                    }
                    _ => {
                        error("Path contains too many elements")
                    }
                }
            } else {
                error("Unexpected cell type, expected field")
            }
        }
        _ => error("Expected exactly one argument, a field defintition")
    }
}

macro_rules! sum_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident) => {
fn $name(mut s: Box<dyn Readable>, column: usize) -> CrushResult<Value> {
    let mut res: $var_type = $var_initializer;
    while let Ok(row) = s.read() {
match row.cells()[column] {
                Value::$value_type(i) => res = res + i,
                _ => return error("Invalid cell value, expected an integer")
            }
    }
    Ok(Value::$value_type(res))
}
    }
}

sum_function!(sum_int, i128, 0, Integer);
sum_function!(sum_float, f64, 0.0, Float);
sum_function!(sum_duration, Duration, Duration::seconds(0), Duration);

pub fn sum(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(input) => {
            let column = parse(input.types(), &context.arguments)?;
            match input.types()[column].cell_type {
                ValueType::Integer => context.output.send(sum_int(input, column)?),
                ValueType::Float => context.output.send(sum_float(input, column)?),
                ValueType::Duration => context.output.send(sum_duration(input, column)?),
                _ => argument_error("")
            }
        }
        _ => error("Expected a stream"),
    }
}

macro_rules! avg_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident, $count_type:ident) => {
fn $name(mut s: Box<dyn Readable>, column: usize) -> CrushResult<Value> {
    let mut res: $var_type = $var_initializer;
    let mut count: i128 = 0;
    loop {
        match s.read() {
            Ok(row) => {
                count += 1;
                match row.cells()[column] {
                    Value::$value_type(i) => res = res + i,
                    _ => return error("Invalid cell value, expected an integer")
                }
            }
            Err(_) => break,
        }
    }
    Ok(Value::$value_type(res / (count as $count_type)))
}
    }
}

avg_function!(avg_int, i128, 0, Integer, i128);
avg_function!(avg_float, f64, 0.0, Float, f64);
avg_function!(avg_duration, Duration, Duration::seconds(0), Duration, i32);

pub fn avg(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(input) => {
            let column = parse(input.types(), &context.arguments)?;
            match input.types()[column].cell_type {
                ValueType::Integer => context.output.send(avg_int(input, column)?),
                ValueType::Float => context.output.send(avg_float(input, column)?),
                ValueType::Duration => context.output.send(avg_duration(input, column)?),
                _ => argument_error("")
            }
        }
        _ => error("Expected a stream"),
    }
}
