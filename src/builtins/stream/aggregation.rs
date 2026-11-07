use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::command::{OutputType, PipeInputType};
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::ColumnVec;
use crate::lang::errors::{CrushResult, command_error, error};
use crate::lang::pipe::Stream;
use crate::lang::state::contexts::CommandContext;
use crate::lang::{value::Value, value::ValueType};
use crate::util::replace::Replace;
use chrono::Duration;
use float_ord::FloatOrd;
use signature::signature;
use std::ops::Deref;

fn parse(input_type: &[ColumnType], field: Option<String>) -> CrushResult<usize> {
    field.map(|f| input_type.find(&f)).unwrap_or_else(|| {
        if input_type.len() == 1 {
            Ok(0)
        } else {
            error("Input stream has multiple columns, you must specify which column to operate on.")
        }
    })
}

/// The input's own columns, if its type is known and table-shaped -- every aggregation
/// function's `dynamic_output` needs these to resolve its `field` argument against.
fn input_columns(input: &PipeInputType) -> Option<&[ColumnType]> {
    match input {
        PipeInputType::Known(
            ValueType::TableInputStream(c) | ValueType::TableOutputStream(c) | ValueType::Table(c),
        ) => Some(c),
        _ => None,
    }
}

/// Resolves every aggregation function's single optional `field` argument against the
/// input's real columns, mirroring `parse()`'s own logic (a given name must exist; no
/// name at all only works when there's exactly one column) -- shared since `sum`, `avg`,
/// `median`, `min`, `max`, `prod`, and `concat` all take this exact argument the same
/// way. Only the *first* unnamed argument is ever considered (matching `field` always
/// being each struct's first declared field): `concat`'s own `separator` is also
/// unnamed, and must never be mistaken for a column name. Returns `Ok(None)` when
/// `field`'s value isn't resolved yet (the caller should fall back to `Unknown`), not an
/// error -- only a *known*, nonexistent column name is a real error.
fn resolve_field_column<'a>(
    columns: &'a [ColumnType],
    arguments: &[ValidationArgument],
) -> CrushResult<Option<&'a ColumnType>> {
    let mut seen_positional = false;
    for arg in arguments {
        let is_field = match &arg.argument_type {
            Some(name) => name == "field",
            None => {
                let first = !seen_positional;
                seen_positional = true;
                first
            }
        };
        if !is_field {
            continue;
        }
        return match &arg.value {
            ArgumentValue::Value(Value::String(s)) => match columns.find(s) {
                Ok(idx) => Ok(Some(&columns[idx])),
                Err(_) => arg.error(format!("Unknown column `{}`", s)),
            },
            ArgumentValue::Unknown | ArgumentValue::Type(_) => Ok(None),
            _ => arg.error("`field` must be given a string value"),
        };
    }
    if columns.len() == 1 {
        Ok(Some(&columns[0]))
    } else {
        command_error(
            "Input stream has multiple columns, you must specify which column to operate on.",
        )
    }
}

/// Shared by `sum`/`avg`'s own `dynamic_output`: both accept exactly the same column
/// types (numeric or duration, or `any` -- whose real result can't be known without
/// reading the actual rows) and differ only in their error message's verb.
fn numeric_or_duration_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
    verb: &str,
) -> CrushResult<OutputType> {
    let Some(columns) = input_columns(&input) else {
        return Ok(OutputType::Unknown);
    };
    let Some(column) = resolve_field_column(columns, arguments)? else {
        return Ok(OutputType::Unknown);
    };
    match &column.cell_type {
        ValueType::Integer | ValueType::Float | ValueType::Duration => {
            Ok(OutputType::Known(column.cell_type.clone()))
        }
        ValueType::Any => Ok(OutputType::Unknown),
        t => command_error(format!("Can't calculate {} of elements of type {}", verb, t)),
    }
}

macro_rules! sum_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: $var_type = $var_initializer;
            while let Some(row) = s.next_row()? {
                match &row.cells()[column] {
                    Value::$value_type(i) => res = res + *i,
                    v => return error(format!("Invalid cell value type `{}`.", v.value_type())),
                }
            }
            Ok(Value::$value_type(res))
        }
    };
}

sum_function!(sum_int, i128, 0, Integer);
sum_function!(sum_float, f64, 0.0, Float);
sum_function!(sum_duration, Duration, Duration::seconds(0), Duration);

fn sum_any(mut s: Stream, column: usize) -> CrushResult<Value> {
    let mut had_integer = false;
    let mut had_float = false;
    let mut had_duration = false;
    let mut sum_int = 0;
    let mut sum_float = 0.0;
    let mut sum_duration = Duration::seconds(0);
    while let Some(row) = s.next_row()? {
        match &row.cells()[column] {
            Value::Integer(i) => {
                had_integer = true;
                sum_int = sum_int + *i
            }
            Value::Float(f) => {
                had_float = true;
                sum_float = sum_float + *f
            }
            Value::Duration(d) => {
                had_duration = true;
                sum_duration = sum_duration + *d
            }
            v => return error(format!("Invalid cell value type `{}`.", v.value_type())),
        }
    }
    match (had_integer, had_float, had_duration) {
        (true, false, false) => Ok(Value::Integer(sum_int)),
        (false, true, false) => Ok(Value::Float(sum_float)),
        (false, false, true) => Ok(Value::Duration(sum_duration)),
        (false, false, false) => error("Sum of nothing"),
        _ => error("Received multiple types in sum"),
    }
}

fn sum_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    numeric_or_duration_dynamic_output(input, arguments, "sum")
}

#[signature(
    stream.sum,
    short = "Calculate the sum for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "",
    long = "The column contents must be exactly one of the types `$integer`, `$float`, or `$duration`.",
    long = "",
    long = "Normally, `sum` expects the type of the column to be the type to sum over, but `sum` can also sum over a column of type `any`, so long as there is at least one row in the table, and all the rows are actually of the same type.",
    example = "$host.procs | sum cpu",
    dynamic_output = sum_dynamic_output)]
pub struct Sum {
    #[description("The name of the column to find the sum of")]
    field: Option<String>,
}

fn sum(mut context: CommandContext) -> CrushResult<()> {
    let input = context.input_stream()?;
    let cfg: Sum = Sum::parse(context.remove_arguments(), &context.global_state.printer())?;
    let column = parse(input.types(), cfg.field)?;
    match &input.types()[column].cell_type {
        ValueType::Integer => context.output.send(sum_int(input, column)?),
        ValueType::Float => context.output.send(sum_float(input, column)?),
        ValueType::Duration => context.output.send(sum_duration(input, column)?),
        ValueType::Any => context.output.send(sum_any(input, column)?),
        t => command_error(&format!("Can't calculate sum of elements of type {}", t)),
    }
}

macro_rules! avg_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident, $count_type:ident) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: $var_type = $var_initializer;
            let mut count: i128 = 0;
            loop {
                match s.next_row()? {
                    Some(row) => {
                        count += 1;
                        match row.cells()[column] {
                            Value::$value_type(i) => res = res + i,
                            _ => return error("Invalid cell value"),
                        }
                    }
                    None => break,
                }
            }
            if count == 0 {
                error("Can't calculate average of empty set")
            } else {
                Ok(Value::$value_type(res / (count as $count_type)))
            }
        }
    };
}

avg_function!(avg_int, i128, 0, Integer, i128);
avg_function!(avg_float, f64, 0.0, Float, f64);
avg_function!(avg_duration, Duration, Duration::seconds(0), Duration, i32);

fn avg_any(mut s: Stream, column: usize) -> CrushResult<Value> {
    let mut had_integer = false;
    let mut had_float = false;
    let mut had_duration = false;
    let mut sum_int = 0;
    let mut sum_float = 0.0;
    let mut sum_duration = Duration::seconds(0);
    let mut count: usize = 0;
    while let Some(row) = s.next_row()? {
        match &row.cells()[column] {
            Value::Integer(i) => {
                had_integer = true;
                sum_int += *i
            }
            Value::Float(f) => {
                had_float = true;
                sum_float += *f
            }
            Value::Duration(d) => {
                had_duration = true;
                sum_duration += *d
            }
            v => return error(format!("Invalid cell value type `{}`.", v.value_type())),
        }
        count += 1;
    }
    match (had_integer, had_float, had_duration) {
        (true, false, false) => Ok(Value::Integer(sum_int / count as i128)),
        (false, true, false) => Ok(Value::Float(sum_float / count as f64)),
        (false, false, true) => Ok(Value::Duration(sum_duration / count as i32)),
        (false, false, false) => error("Can't calculate average of empty set"),
        _ => error("Received multiple types in average"),
    }
}

fn avg_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    numeric_or_duration_dynamic_output(input, arguments, "average")
}

#[signature(
    stream.avg,
    short = "Calculate the average for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column type must be numeric or a duration.",
    example = "$host.procs | avg cpu",
    dynamic_output = avg_dynamic_output)]
pub struct Avg {
    #[description("The name of the column to find the average of")]
    field: Option<String>,
}

fn avg(mut context: CommandContext) -> CrushResult<()> {
    let input = context.input_stream()?;
    let cfg: Avg = Avg::parse(context.remove_arguments(), &context.global_state.printer())?;
    let column = parse(input.types(), cfg.field)?;
    match &input.types()[column].cell_type {
        ValueType::Integer => context.output.send(avg_int(input, column)?),
        ValueType::Float => context.output.send(avg_float(input, column)?),
        ValueType::Duration => context.output.send(avg_duration(input, column)?),
        ValueType::Any => context.output.send(avg_any(input, column)?),
        t => command_error(&format!(
            "Can't calculate average of elements of type {}",
            t
        )),
    }
}

/// Never NaN: used as the `$is_nan` check for `median_function` instantiations whose
/// value type has no NaN concept (integers, durations).
fn never_nan<T>(_: T) -> bool {
    false
}

macro_rules! median_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident, $count_type:ident, $halver:expr, $is_nan:expr) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: Vec<$var_type> = Vec::new();
            loop {
                match s.next_row()? {
                    Some(row) => match row.cells()[column] {
                        Value::$value_type(i) => {
                            if $is_nan(i) {
                                return command_error(
                                    "Can't calculate median of a set containing NaN values.",
                                );
                            }
                            res.push(i)
                        }
                        _ => return error("Invalid cell value"),
                    },
                    None => break,
                }
            }
            // NaN values were rejected above, so partial_cmp is never None here.
            res.sort_by(|a, b| a.partial_cmp(b).unwrap());
            if (res.is_empty()) {
                command_error("Can't calculate median of empty set")
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

median_function!(median_int, i128, 0, Integer, i128, 2, never_nan);
median_function!(median_float, f64, 0.0, Float, f64, 2.0, f64::is_nan);
median_function!(
    median_duration,
    Duration,
    Duration::seconds(0),
    Duration,
    i32,
    2,
    never_nan
);

fn median_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let Some(columns) = input_columns(&input) else {
        return Ok(OutputType::Unknown);
    };
    let Some(column) = resolve_field_column(columns, arguments)? else {
        return Ok(OutputType::Unknown);
    };
    match &column.cell_type {
        ValueType::Integer | ValueType::Float | ValueType::Duration => {
            Ok(OutputType::Known(column.cell_type.clone()))
        }
        t => command_error(format!("Can't calculate average of elements of type {}", t)),
    }
}

#[signature(
    stream.median,
    short = "Calculate the median for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column type must be numeric or a duration.",
    long = "If the column contains a NaN float value, `median` fails with an error rather \
    than silently including it in (or excluding it from) the calculation.",
    example = "$host.procs | median cpu",
    dynamic_output = median_dynamic_output)]
pub struct Median {
    #[description("The name of the column to find the median of")]
    field: Option<String>,
}

fn median(mut context: CommandContext) -> CrushResult<()> {
    let input = context.input_stream()?;
    let cfg: Median = Median::parse(context.remove_arguments(), &context.global_state.printer())?;
    let column = parse(input.types(), cfg.field)?;
    match &input.types()[column].cell_type {
        ValueType::Integer => {
            context
                .output
                .send(median_int(
                    input, column,
                )?)
        }
        ValueType::Float => {
            context
                .output
                .send(median_float(
                    input, column,
                )?)
        }
        ValueType::Duration => {
            context
                .output
                .send(median_duration(
                    input, column,
                )?)
        }
        t => command_error(&format!(
            "Can't calculate average of elements of type {}",
            t
        )),
    }
}

macro_rules! aggr_function {
    ($name:ident, $value_type:ident, $op_desc:literal, $type_desc:literal, $op:expr) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            if let Some(first_value) = s.next_row()? {
                let mut res = match first_value.into_cells().replace(column, Value::Empty) {
                    Value::$value_type(i) => i,
                    _ => return error(concat!("Invalid cell value, expected ", $type_desc)),
                };
                while let Some(row) = s.next_row()? {
                    match row.into_cells().replace(column, Value::Empty) {
                        Value::$value_type(i) => res = $op(i, res),
                        _ => return error(concat!("Invalid cell value, expected ", $type_desc)),
                    }
                }
                Ok(Value::$value_type(res))
            } else {
                error(concat!("Can't calculate ", $op_desc, " of empty set"))
            }
        }
    };
}

aggr_function!(min_int, Integer, "min", "integer", |a, b| std::cmp::min(a, b));
aggr_function!(min_float, Float, "min", "float", |a, b| std::cmp::min(
    FloatOrd(a),
    FloatOrd(b)
)
.0);
aggr_function!(min_duration, Duration, "min", "duration", |a, b| std::cmp::min(
    a, b
));
aggr_function!(min_time, Time, "min", "time", |a, b| std::cmp::min(a, b));
aggr_function!(min_string, String, "min", "string", |a, b| std::cmp::min(a, b));
aggr_function!(min_file, File, "min", "file", |a, b| std::cmp::min(a, b));

aggr_function!(max_int, Integer, "max", "integer", |a, b| std::cmp::max(a, b));
aggr_function!(max_float, Float, "max", "float", |a, b| std::cmp::max(
    FloatOrd(a),
    FloatOrd(b)
)
.0);
aggr_function!(max_duration, Duration, "max", "duration", |a, b| std::cmp::max(
    a, b
));
aggr_function!(max_time, Time, "max", "time", |a, b| std::cmp::max(a, b));
aggr_function!(max_string, String, "max", "string", |a, b| std::cmp::max(a, b));
aggr_function!(max_file, File, "max", "file", |a, b| std::cmp::max(a, b));

/// Shared by `min`/`max`'s own `dynamic_output`: both accept exactly the same column
/// types and simply echo the column's own type back unchanged.
fn min_max_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let Some(columns) = input_columns(&input) else {
        return Ok(OutputType::Unknown);
    };
    let Some(column) = resolve_field_column(columns, arguments)? else {
        return Ok(OutputType::Unknown);
    };
    match &column.cell_type {
        ValueType::Integer
        | ValueType::Float
        | ValueType::Duration
        | ValueType::Time
        | ValueType::String
        | ValueType::File => Ok(OutputType::Known(column.cell_type.clone())),
        t => command_error(format!(
            "Can't calculate the minimum or maximum of elements of type {}",
            t
        )),
    }
}

#[signature(
    stream.min,
    short = "Calculate the minimum for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column can be numeric, temporal, a string or a file.",
    example = "$host.procs | min cpu",
    dynamic_output = min_max_dynamic_output)]
pub struct Min {
    #[description("The name of the column to find the minimum of")]
    field: Option<String>,
}

fn min(mut context: CommandContext) -> CrushResult<()> {
    let input = context.input_stream()?;
    let cfg: Min = Min::parse(context.remove_arguments(), &context.global_state.printer())?;
    let column = parse(input.types(), cfg.field)?;
    match &input.types()[column].cell_type {
        ValueType::Integer => context.output.send(min_int(input, column)?),
        ValueType::Float => context.output.send(min_float(input, column)?),
        ValueType::Duration => context.output.send(min_duration(input, column)?),
        ValueType::Time => context.output.send(min_time(input, column)?),
        ValueType::String => context.output.send(min_string(input, column)?),
        ValueType::File => context.output.send(min_file(input, column)?),
        t => command_error(&format!("Can't pick min of elements of type {}", t)),
    }
}

#[signature(
    stream.max,
    short = "Calculate the maximum for the specific column across all rows.",
    long = "If the input only has one column, the column name is optional.",
    long = "The column can be numeric, temporal, a string or a file.",
    example = "$host.procs | max cpu",
    dynamic_output = min_max_dynamic_output)]
pub struct Max {
    #[description("The name of the column to find the maximum of")]
    field: Option<String>,
}

fn max(mut context: CommandContext) -> CrushResult<()> {
    let input = context.input_stream()?;
    let cfg: Max = Max::parse(context.remove_arguments(), &context.global_state.printer())?;
    let column = parse(input.types(), cfg.field)?;
    match &input.types()[column].cell_type {
        ValueType::Integer => context.output.send(max_int(input, column)?),
        ValueType::Float => context.output.send(max_float(input, column)?),
        ValueType::Duration => context.output.send(max_duration(input, column)?),
        ValueType::Time => context.output.send(max_time(input, column)?),
        ValueType::String => context.output.send(max_string(input, column)?),
        ValueType::File => context.output.send(max_file(input, column)?),
        t => command_error(&format!("Can't pick max of elements of type {}", t)),
    }
}

macro_rules! prod_function {
    ($name:ident, $var_type:ident, $var_initializer:expr, $value_type:ident) => {
        fn $name(mut s: Stream, column: usize) -> CrushResult<Value> {
            let mut res: $var_type = $var_initializer;
            while let Some(row) = s.next_row()? {
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

fn prod_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let Some(columns) = input_columns(&input) else {
        return Ok(OutputType::Unknown);
    };
    let Some(column) = resolve_field_column(columns, arguments)? else {
        return Ok(OutputType::Unknown);
    };
    match &column.cell_type {
        ValueType::Integer | ValueType::Float => Ok(OutputType::Known(column.cell_type.clone())),
        t => command_error(format!("Can't calculate product of elements of type {}", t)),
    }
}

#[signature(
    stream.prod,
    short = "Calculate the product of the specified column across all rows.",
    long = "Specifying the column is optional if the stream only has one column.",
    long = "The column type must be numeric.",
    example = "seq 5 10 | prod",
    dynamic_output = prod_dynamic_output)]
pub struct Prod {
    #[description("The name of the column to find the product of")]
    field: Option<String>,
}

fn prod(mut context: CommandContext) -> CrushResult<()> {
    let input = context.input_stream()?;
    let cfg = Prod::parse(context.remove_arguments(), &context.global_state.printer())?;
    let column = parse(input.types(), cfg.field)?;
    match &input.types()[column].cell_type {
        ValueType::Integer => context.output.send(prod_int(input, column)?),
        ValueType::Float => context.output.send(prod_float(input, column)?),
        t => command_error(&format!(
            "Can't calculate product of elements of type {}",
            t
        )),
    }
}

/// Unlike every other aggregation function here, `concat` doesn't actually constrain
/// the column's type ahead of time -- it just converts whatever each row's value is
/// (string/file/integer/float) and errors per-row if it's something else. Its output is
/// always a plain string, as long as the column it names actually exists.
fn concat_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let Some(columns) = input_columns(&input) else {
        return Ok(OutputType::Unknown);
    };
    match resolve_field_column(columns, arguments)? {
        Some(_) => Ok(OutputType::Known(ValueType::String)),
        None => Ok(OutputType::Unknown),
    }
}

#[signature(
    stream.concat,
    short = "Concatenate all values of the specified column across all rows",
    long = "If the input only has one column, the column name is optional.",
    long = "The column can be numeric, or textual.",
    example = "$host.procs | concat name \":\"",
    dynamic_output = concat_dynamic_output,
)]
pub struct Concat {
    #[description("The name of the column to concatenate")]
    field: Option<String>,
    #[description("The separator to insert between each element")]
    #[default(", ")]
    separator: String,
}

fn concat(mut context: CommandContext) -> CrushResult<()> {
    let mut input = context.input_stream()?;
    let cfg: Concat = Concat::parse(context.remove_arguments(), &context.global_state.printer())?;
    let column = parse(input.types(), cfg.field)?;
    let mut res = String::new();

    if let Some(row) = input.next_row()? {
        match row.into_cells().replace(column, Value::Empty) {
            Value::String(i) => res.push_str(i.deref()),
            Value::File(i) => res.push_str(i.to_str().unwrap_or("<Invalid>")),
            Value::Integer(i) => res.push_str(&i.to_string()),
            Value::Float(i) => res.push_str(&i.to_string()),
            _ => return error("Invalid cell value, expected number or text"),
        };
        while let Some(row) = input.next_row()? {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use std::sync::Arc;

    fn unnamed_arg(value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: None,
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    fn named_arg(name: &str, value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: Some(name.to_string()),
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    fn known(columns: Vec<ColumnType>) -> PipeInputType {
        PipeInputType::Known(ValueType::TableInputStream(columns))
    }

    fn one_column(t: ValueType) -> Vec<ColumnType> {
        vec![ColumnType::new("value", t, "")]
    }

    fn two_columns() -> Vec<ColumnType> {
        vec![
            ColumnType::new("cpu", ValueType::Float, ""),
            ColumnType::new("name", ValueType::String, ""),
        ]
    }

    #[test]
    fn sum_of_integer_column_is_integer() {
        let out = sum_dynamic_output(known(one_column(ValueType::Integer)), &[]).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Integer)));
    }

    #[test]
    fn sum_of_any_column_is_unknown() {
        let out = sum_dynamic_output(known(one_column(ValueType::Any)), &[]).unwrap();
        assert!(matches!(out, OutputType::Unknown));
    }

    #[test]
    fn sum_of_unsupported_column_is_an_error() {
        assert!(sum_dynamic_output(known(one_column(ValueType::String)), &[]).is_err());
    }

    #[test]
    fn sum_without_field_on_multiple_columns_is_an_error() {
        assert!(sum_dynamic_output(known(two_columns()), &[]).is_err());
    }

    #[test]
    fn sum_with_field_picks_the_named_column() {
        let args = [named_arg("field", ArgumentValue::Value(Value::from("cpu")))];
        let out = sum_dynamic_output(known(two_columns()), &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Float)));
    }

    #[test]
    fn sum_with_unknown_field_is_an_error() {
        let args = [named_arg(
            "field",
            ArgumentValue::Value(Value::from("nonexistent")),
        )];
        assert!(sum_dynamic_output(known(two_columns()), &args).is_err());
    }

    #[test]
    fn avg_of_duration_column_is_duration() {
        let out = avg_dynamic_output(known(one_column(ValueType::Duration)), &[]).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Duration)));
    }

    #[test]
    fn median_of_float_column_is_float() {
        let out = median_dynamic_output(known(one_column(ValueType::Float)), &[]).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Float)));
    }

    #[test]
    fn median_of_unsupported_column_is_an_error() {
        assert!(median_dynamic_output(known(one_column(ValueType::String)), &[]).is_err());
    }

    #[test]
    fn min_max_of_string_column_is_string() {
        let out = min_max_dynamic_output(known(one_column(ValueType::String)), &[]).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::String)));
    }

    #[test]
    fn min_max_of_unsupported_column_is_an_error() {
        assert!(min_max_dynamic_output(known(one_column(ValueType::Any)), &[]).is_err());
    }

    #[test]
    fn prod_of_integer_column_is_integer() {
        let out = prod_dynamic_output(known(one_column(ValueType::Integer)), &[]).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::Integer)));
    }

    #[test]
    fn prod_of_duration_column_is_an_error() {
        assert!(prod_dynamic_output(known(one_column(ValueType::Duration)), &[]).is_err());
    }

    #[test]
    fn concat_is_always_string() {
        let out = concat_dynamic_output(known(one_column(ValueType::Integer)), &[]).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::String)));
    }

    #[test]
    fn concat_separator_positional_argument_is_not_mistaken_for_the_field() {
        // `concat name ":"` -- the first unnamed arg (the field name) must win, not the
        // second (the separator), even though both are plain unnamed string arguments.
        let args = [
            unnamed_arg(ArgumentValue::Value(Value::from("name"))),
            unnamed_arg(ArgumentValue::Value(Value::from(":"))),
        ];
        let out = concat_dynamic_output(known(two_columns()), &args).unwrap();
        assert!(matches!(out, OutputType::Known(ValueType::String)));
    }

    #[test]
    fn unresolved_field_falls_back_to_unknown() {
        let args = [named_arg("field", ArgumentValue::Unknown)];
        let out = sum_dynamic_output(known(two_columns()), &args).unwrap();
        assert!(matches!(out, OutputType::Unknown));
    }

    #[test]
    fn unknown_input_type_is_unknown() {
        let out = sum_dynamic_output(PipeInputType::Unknown, &[]).unwrap();
        assert!(matches!(out, OutputType::Unknown));
    }
}
