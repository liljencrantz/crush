use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnVec;
use crate::lang::errors::{CrushResult, error, argument_error};
use crate::lang::pipe::Stream;
use crate::lang::state::contexts::CommandContext;
use crate::lang::{value::Value, value::ValueType};
use crate::util::replace::Replace;
use chrono::Duration;
use float_ord::FloatOrd;
use signature::signature;
use std::ops::Deref;
use crate::lang::ast::source::Source;

fn parse(command_name: &str, input_type: &[ColumnType], field: Option<String>) -> CrushResult<usize> {
    field.map(|f| input_type.find(&f)).unwrap_or_else(|| {
        if input_type.len() == 1 {
            Ok(0)
        } else {
            error(format!("`{}`: Input stream has multiple columns, you must specify which column to operate on.", command_name))
        }
    })
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
    stream.sum,
    short = "Calculate the sum for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column type must be numeric or a duration.",
    example = "host:procs | sum cpu")]
pub struct Sum {
    #[description("The name of the column to find the sum of")]
    field: Option<String>,
}

fn sum(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Sum = Sum::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
            let column = parse("sum", input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(sum_int(input, column)?),
                ValueType::Float => context.output.send(sum_float(input, column)?),
                ValueType::Duration => context.output.send(sum_duration(input, column)?),
                t => {
                    argument_error(&format!("Can't calculate sum of elements of type {}", t), &context.source)
                }
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
    stream.avg,
    short = "Calculate the average for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column type must be numeric or a duration.",
    example = "host:procs | avg cpu")]
pub struct Avg {
    #[description("The name of the column to find the average of")]
    field: Option<String>,
}

fn avg(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Avg = Avg::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
            let column = parse("avg", input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(avg_int(input, column)?),
                ValueType::Float => context.output.send(avg_float(input, column)?),
                ValueType::Duration => context.output.send(avg_duration(input, column)?),
                t => argument_error(&format!(
                    "Can't calculate average of elements of type {}",
                    t
                ), &context.source),
            }
        }
        _ => error("Expected a stream"),
    }
}

macro_rules! median_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident, $count_type:ident, $halver:expr) => {
        fn $name(source: &Source, mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: Vec<$var_type> = Vec::new();
            loop {
                match s.read() {
                    Ok(row) => match row.cells()[column] {
                        Value::$value_type(i) => res.push(i),
                        _ => return error("Invalid cell value"),
                    },
                    Err(_) => break,
                }
            }
            res.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if (res.is_empty()) {
                argument_error("Can't calculate median of empty set", source)
            } else if (res.len() % 2 == 1) {
                Ok(Value::$value_type(res[(res.len() - 1) / 2]))
            } else {
                let low = res[(res.len() / 2) - 1];
                let high = res[(res.len() / 2)];
                Ok(Value::$value_type((low + high) / $halver))
            }
        }
    };
}

median_function!(median_int, i128, 0, Integer, i128, 2);
median_function!(median_float, f64, 0.0, Float, f64, 2.0);
median_function!(
    median_duration,
    Duration,
    Duration::seconds(0),
    Duration,
    i32,
    2
);

#[signature(
    stream.median,
    short = "Calculate the median for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column type must be numeric or a duration.",
    example = "host:procs | median cpu")]
pub struct Median {
    #[description("The name of the column to find the median of")]
    field: Option<String>,
}

fn median(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Median = Median::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
            let column = parse("median", input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => {
                    context
                        .output
                        .send(crate::builtins::stream::aggregation::median_int(
                            &context.source, input, column,
                        )?)
                }
                ValueType::Float => {
                    context
                        .output
                        .send(crate::builtins::stream::aggregation::median_float(
                            &context.source, input, column,
                        )?)
                }
                ValueType::Duration => {
                    context
                        .output
                        .send(crate::builtins::stream::aggregation::median_duration(
                            &context.source, input, column,
                        )?)
                }
                t => argument_error(&format!(
                    "Can't calculate average of elements of type {}",
                    t
                ), &context.source),
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
aggr_function!(min_duration, Duration, "duration", |a, b| std::cmp::min(
    a, b
));
aggr_function!(min_time, Time, "time", |a, b| std::cmp::min(a, b));
aggr_function!(min_string, String, "string", |a, b| std::cmp::min(a, b));
aggr_function!(min_file, File, "file", |a, b| std::cmp::min(a, b));

aggr_function!(max_int, Integer, "integer", |a, b| std::cmp::max(a, b));
aggr_function!(max_float, Float, "float", |a, b| std::cmp::max(
    FloatOrd(a),
    FloatOrd(b)
)
.0);
aggr_function!(max_duration, Duration, "duration", |a, b| std::cmp::max(
    a, b
));
aggr_function!(max_time, Time, "time", |a, b| std::cmp::max(a, b));
aggr_function!(max_string, String, "string", |a, b| std::cmp::max(a, b));
aggr_function!(max_file, File, "file", |a, b| std::cmp::max(a, b));

#[signature(
    stream.min,
    short = "Calculate the minimum for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column can be numeric, temporal, a string or a file.",
    example = "host:procs | min cpu")]
pub struct Min {
    #[description("The name of the column to find the minimum of")]
    field: Option<String>,
}

fn min(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Min = Min::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
            let column = parse("min", input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(min_int(input, column)?),
                ValueType::Float => context.output.send(min_float(input, column)?),
                ValueType::Duration => context.output.send(min_duration(input, column)?),
                ValueType::Time => context.output.send(min_time(input, column)?),
                ValueType::String => context.output.send(min_string(input, column)?),
                ValueType::File => context.output.send(min_file(input, column)?),
                t => argument_error(&format!("Can't pick min of elements of type {}", t), &context.source),
            }
        }
        _ => error("Expected a stream"),
    }
}

#[signature(
    stream.max,
    short = "Calculate the maximum for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column can be numeric, temporal, a string or a file.",
    example = "host:procs | max cpu")]
pub struct Max {
    #[description("The name of the column to find the maximum of")]
    field: Option<String>,
}

fn max(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg: Max = Max::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
            let column = parse("max", input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(max_int(input, column)?),
                ValueType::Float => context.output.send(max_float(input, column)?),
                ValueType::Duration => context.output.send(max_duration(input, column)?),
                ValueType::Time => context.output.send(max_time(input, column)?),
                ValueType::String => context.output.send(max_string(input, column)?),
                ValueType::File => context.output.send(max_file(input, column)?),
                t => argument_error(&format!("Can't pick max of elements of type {}", t), &context.source),
            }
        }
        _ => error("Expected a stream"),
    }
}

macro_rules! prod_function {
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

prod_function!(prod_int, i128, 1, Integer);
prod_function!(prod_float, f64, 1.0, Float);

#[signature(
    stream.prod,
    short = "Calculate the product of the specified column across all rows.",
    long = "Specifying the column is optional if the stream only has one column.",
    long = "The column type must be numeric.",
    example = "seq 5 10 | prod")]
pub struct Prod {
    #[description("The name of the column to find the product of")]
    field: Option<String>,
}

fn prod(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(input) => {
            let cfg = Prod::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
            let column = parse("prod", input.types(), cfg.field)?;
            match &input.types()[column].cell_type {
                ValueType::Integer => context.output.send(prod_int(input, column)?),
                ValueType::Float => context.output.send(prod_float(input, column)?),
                t => argument_error(&format!(
                    "Can't calculate product of elements of type {}",
                    t
                ), &context.source),
            }
        }
        _ => error("Expected a stream"),
    }
}

#[signature(
    stream.concat,
    short = "Concatenate all values of the specified column across all rows",
    long = "If the input only has one column, the column name is optional.",
    long = "The column can be numeric, or textual.",
    example = "host:procs | concat name \":\"",
)]
pub struct Concat {
    #[description("The name of the column to concatenate")]
    field: Option<String>,
    #[description("The separator to insert between each element")]
    #[default(", ")]
    separator: String,
}

fn concat(mut context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let cfg: Concat = Concat::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
            let column = parse("concat", input.types(), cfg.field)?;
            let mut res = String::new();

            if let Ok(row) = input.read() {
                match row.into_cells().replace(column, Value::Empty) {
                    Value::String(i) => res.push_str(i.deref()),
                    Value::File(i) => res.push_str(i.to_str().unwrap_or("<Invalid>")),
                    Value::Integer(i) => res.push_str(&i.to_string()),
                    Value::Float(i) => res.push_str(&i.to_string()),
                    _ => return error("Invalid cell value, expected number or text"),
                };
                while let Ok(row) = input.read() {
                    res.push_str(&cfg.separator);
                    match row.into_cells().replace(column, Value::Empty) {
                        Value::String(i) => res.push_str(i.deref()),
                        Value::File(i) => res.push_str(i.to_str().unwrap_or("<Invalid>")),
                        Value::Integer(i) => res.push_str(&i.to_string()),
                        Value::Float(i) => res.push_str(&i.to_string()),
                        _ => return error("Invalid cell value, expected number or text"),
                    }
                }
            }
            context.output.send(Value::from(res))
        }
        _ => error("Expected a stream"),
    }
}
