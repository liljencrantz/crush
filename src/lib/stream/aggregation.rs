use crate::lang::errors::{argument_error_legacy, error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::pipe::Stream;
use crate::lang::data::table::ColumnVec;
use crate::lang::{data::table::ColumnType};
use crate::lang::{value::Value, value::ValueType};
use chrono::Duration;
use float_ord::FloatOrd;
use signature::signature;
use crate::util::replace::Replace;

fn parse(input_type: &[ColumnType], field: Option<String>) -> CrushResult<usize> {
    field.map(|f| input_type.find(&f))
        .unwrap_or_else(||
            if input_type.len() == 1 {
                Ok(0)
            } else {
                error("Specify which column to operate on")
            }
        )
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

#[signature(
sum,
short = "Calculate the sum for the specific column across all rows.",
example = "proc:list | sum cpu")]
pub struct Sum {
    field: Option<String>,
}

fn sum(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Sum = Sum::parse(context.arguments, &context.global_state.printer())?;
            let column = parse(input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(sum_int(input, column)?),
                ValueType::Float => context.output.send(sum_float(input, column)?),
                ValueType::Duration => context.output.send(sum_duration(input, column)?),
                t => argument_error_legacy(
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

#[signature(
avg,
short = "Calculate the average for the specific column across all rows.",
example = "proc:list | avg cpu")]
pub struct Avg {
    field: Option<String>,
}

fn avg(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Avg = Avg::parse(context.arguments, &context.global_state.printer())?;
            let column = parse(input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(avg_int(input, column)?),
                ValueType::Float => context.output.send(avg_float(input, column)?),
                ValueType::Duration => context.output.send(avg_duration(input, column)?),
                t => argument_error_legacy(
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
    ($name:ident, $value_type:ident, $desc:literal, $op:expr) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res = match s.read()?.into_cells().replace(column, Value::Empty) {
                Value::$value_type(i) => i,
                _ => return error(concat!("Invalid cell value, expected ", $desc)),
            };
            while let Ok(row) = s.read() {
                match row.into_cells().replace(column, Value::Empty) {
                    Value::$value_type(i) => res = $op(i, res),
                    _ => return error(concat!("Invalid cell value, expected ", $desc)),
                }
            }
            Ok(Value::$value_type(res))
        }
    };
}

aggr_function!(min_int, Integer, "integer", |a, b| std::cmp::min(a, b));
aggr_function!(min_float, Float, "float", |a, b| std::cmp::min(
    FloatOrd(a),
    FloatOrd(b)
)
.0);
aggr_function!(min_duration, Duration, "duration", |a, b| std::cmp::min(a, b));
aggr_function!(min_time, Time, "time", |a, b| std::cmp::min(a, b));
aggr_function!(min_string, String, "string", |a, b| std::cmp::min(a, b));
aggr_function!(min_file, File, "file", |a, b| std::cmp::min(a, b));


aggr_function!(max_int, Integer, "integer", |a, b| std::cmp::max(a, b));
aggr_function!(max_float, Float, "float", |a, b| std::cmp::max(
    FloatOrd(a),
    FloatOrd(b)
)
.0);
aggr_function!(max_duration, Duration, "duration", |a, b| std::cmp::max(a, b));
aggr_function!(max_time, Time, "time", |a, b| std::cmp::max(a, b));
aggr_function!(max_string, String, "string", |a, b| std::cmp::max(a, b));
aggr_function!(max_file, File, "file", |a, b| std::cmp::max(a, b));

#[signature(
min,
short = "Calculate the minimum for the specific column across all rows.",
long = "If the input only has one column, the column name is optional.\n\n    The column can be numeric, temporal, a string or a file.",
example = "host:procs | min cpu")]
pub struct Min {
    field: Option<String>,
}

fn min(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Min = Min::parse(context.arguments, &context.global_state.printer())?;
            let column = parse(input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(min_int(input, column)?),
                ValueType::Float => context.output.send(min_float(input, column)?),
                ValueType::Duration => context.output.send(min_duration(input, column)?),
                ValueType::Time => context.output.send(min_time(input, column)?),
                ValueType::String => context.output.send(min_string(input, column)?),
                ValueType::File => context.output.send(min_file(input, column)?),
                t => argument_error_legacy(
                    &format!("Can't pick min of elements of type {}", t),
                ),
            }
        }
        _ => error("Expected a stream"),
    }
}

#[signature(
max,
short = "Calculate the maximum for the specific column across all rows.",
long = "If the input only has one column, the column name is optional.\n\n    The column can be numeric, temporal, a string or a file.",
example = "host:procs | max cpu")]
pub struct Max {
    field: Option<String>,
}

fn max(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Max = Max::parse(context.arguments, &context.global_state.printer())?;
            let column = parse(input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(max_int(input, column)?),
                ValueType::Float => context.output.send(max_float(input, column)?),
                ValueType::Duration => context.output.send(max_duration(input, column)?),
                ValueType::Time => context.output.send(max_time(input, column)?),
                ValueType::String => context.output.send(max_string(input, column)?),
                ValueType::File => context.output.send(max_file(input, column)?),
                t => argument_error_legacy(
                    &format!("Can't pick max of elements of type {}", t),
                ),
            }
        }
        _ => error("Expected a stream"),
    }
}

macro_rules! mul_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: $var_type = $var_initializer;
            while let Ok(row) = s.read() {
                match row.cells()[column] {
                    Value::$value_type(i) => res = res * i,
                    _ => return error("Invalid cell value"),
                }
            }
            Ok(Value::$value_type(res))
        }
    };
}

mul_function!(mul_int, i128, 1, Integer);
mul_function!(mul_float, f64, 1.0, Float);

#[signature(
mul,
short = "Calculate the product for the specific column across all rows.",
example = "seq 5 10 | mul")]
pub struct Mul {
    field: Option<String>,
}

fn mul(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg = Mul::parse(context.arguments, &context.global_state.printer())?;
            let column = parse(input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(mul_int(input, column)?),
                ValueType::Float => context.output.send(mul_float(input, column)?),
                t => argument_error_legacy(
                    &format!("Can't calculate product of elements of type {}", t),
                ),
            }
        }
        _ => error("Expected a stream"),
    }
}

# [signature(
first,
short = "Return the value of the specified column from the first row of the stream.",
)]
pub struct First {
    field: Option<String>,
}

fn first(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let cfg: First = First::parse(context.arguments, &context.global_state.printer())?;
            let column = parse(input.types(), cfg.field)?;

            if let Ok(mut row) = input.read() {
                context.output.send(row.into_cells().replace(column, Value::Empty).clone())
            } else {
                error("Empty stream")
            }
        }
        _ => error("Expected a stream"),
    }
}

# [signature(
last,
short = "Return the value of the specified column from the last row of the stream.",
)]
pub struct Last {
    field: Option<String>,
}

fn last(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let cfg: First = First::parse(context.arguments, &context.global_state.printer())?;
            let column = parse(input.types(), cfg.field)?;

            let mut rr = None;
            while let Ok(mut row) = input.read() {
                rr = Some(row)
            }
            rr
                .map(|r| {context.output.send(r.into_cells().replace(column, Value::Empty))})
                .unwrap_or_else(|| {argument_error_legacy("Empty stream")})
        }
        _ => error("Expected a stream"),
    }
}
