use crate::lang::errors::{argument_error, error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::stream::Stream;
use crate::lang::data::table::ColumnVec;
use crate::lang::{argument::Argument, data::table::ColumnType};
use crate::lang::{value::Value, value::ValueType};
use chrono::Duration;
use float_ord::FloatOrd;

fn parse(input_type: &[ColumnType], arguments: &[Argument]) -> CrushResult<usize> {
    match arguments.len() {
        0 => {
            if input_type.len() == 1 {
                Ok(0)
            } else {
                error("Specify which column to operate on")
            }
        }
        1 => {
            if let Value::Field(f) = &arguments[0].value {
                match f.len() {
                    1 => Ok(input_type.find_str(f[0].as_ref())?),
                    _ => error("Path contains too many elements"),
                }
            } else {
                error("Unexpected cell type, expected field")
            }
        }
        _ => error("Expected exactly one argument, a field definition"),
    }
}

macro_rules! sum_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: $var_type = $var_initializer;
            while let Ok(row) = s.read() {
                match row.cells()[column] {
                    Value::$value_type(i) => res = res + i,
                    _ => return error("Invalid cell value"),
                }
            }
            Ok(Value::$value_type(res))
        }
    };
}

sum_function!(sum_int, i128, 0, Integer);
sum_function!(sum_float, f64, 0.0, Float);
sum_function!(sum_duration, Duration, Duration::seconds(0), Duration);

pub fn sum(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream() {
        Some(input) => {
            let column = parse(input.types(), &context.arguments)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(sum_int(input, column)?),
                ValueType::Float => context.output.send(sum_float(input, column)?),
                ValueType::Duration => context.output.send(sum_duration(input, column)?),
                t => argument_error(
                    &format!("Can't calculate sum of elements of type {}", t),
                ),
            }
        }
        _ => error("Expected a stream"),
    }
}

macro_rules! avg_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident, $count_type:ident) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: $var_type = $var_initializer;
            let mut count: i128 = 0;
            loop {
                match s.read() {
                    Ok(row) => {
                        count += 1;
                        match row.cells()[column] {
                            Value::$value_type(i) => res = res + i,
                            _ => return error("Invalid cell value"),
                        }
                    }
                    Err(_) => break,
                }
            }
            Ok(Value::$value_type(res / (count as $count_type)))
        }
    };
}

avg_function!(avg_int, i128, 0, Integer, i128);
avg_function!(avg_float, f64, 0.0, Float, f64);
avg_function!(avg_duration, Duration, Duration::seconds(0), Duration, i32);

pub fn avg(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream() {
        Some(input) => {
            let column = parse(input.types(), &context.arguments)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(avg_int(input, column)?),
                ValueType::Float => context.output.send(avg_float(input, column)?),
                ValueType::Duration => context.output.send(avg_duration(input, column)?),
                t => argument_error(
                    &format!(
                        "Can't calculate average of elements of type {}",
                        t
                    ),
                ),
            }
        }
        _ => error("Expected a stream"),
    }
}

macro_rules! aggr_function {
    ($name:ident, $value_type:ident, $op:expr) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res = match s.read()?.cells()[column] {
                Value::$value_type(i) => i,
                _ => return error("Invalid cell value, expected an integer"),
            };
            while let Ok(row) = s.read() {
                match row.cells()[column] {
                    Value::$value_type(i) => res = $op(res, i),
                    _ => return error("Invalid cell value, expected an integer"),
                }
            }
            Ok(Value::$value_type(res))
        }
    };
}

aggr_function!(min_int, Integer, |a, b| std::cmp::min(a, b));
aggr_function!(min_float, Float, |a, b| std::cmp::min(
    FloatOrd(a),
    FloatOrd(b)
)
.0);
aggr_function!(min_duration, Duration, |a, b| std::cmp::min(a, b));
aggr_function!(min_time, Time, |a, b| std::cmp::min(a, b));

aggr_function!(max_int, Integer, |a, b| std::cmp::max(a, b));
aggr_function!(max_float, Float, |a, b| std::cmp::max(
    FloatOrd(a),
    FloatOrd(b)
)
.0);
aggr_function!(max_duration, Duration, |a, b| std::cmp::max(a, b));
aggr_function!(max_time, Time, |a, b| std::cmp::max(a, b));

pub fn min(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream() {
        Some(input) => {
            let column = parse(input.types(), &context.arguments)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(min_int(input, column)?),
                ValueType::Float => context.output.send(min_float(input, column)?),
                ValueType::Duration => context.output.send(min_duration(input, column)?),
                ValueType::Time => context.output.send(min_time(input, column)?),
                t => argument_error(
                    &format!("Can't pick min of elements of type {}", t),
                ),
            }
        }
        _ => error("Expected a stream"),
    }
}

pub fn max(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream() {
        Some(input) => {
            let column = parse(input.types(), &context.arguments)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(max_int(input, column)?),
                ValueType::Float => context.output.send(max_float(input, column)?),
                ValueType::Duration => context.output.send(max_duration(input, column)?),
                ValueType::Time => context.output.send(max_time(input, column)?),
                t => argument_error(
                    &format!("Can't pick max of elements of type {}", t),
                ),
            }
        }
        _ => error("Expected a stream"),
    }
}
