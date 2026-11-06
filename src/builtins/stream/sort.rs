use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::command::OutputType::Passthrough;
use crate::lang::command::{OutputType, PipeInputType};
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushResult;
use crate::lang::errors::command_error;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ComparisonMode::{CaseInsensitive, Regular};
use crate::lang::value::ValueType;
use crate::lang::value::{ComparisonMode, Value};
use signature::signature;
use std::cmp::Ordering;

/// Like `Value::param_partial_cmp`, but never returns `None`. The only comparable value
/// that can make `param_partial_cmp` return `None` is a NaN `Float` (`is_comparable()`
/// only excludes whole *types*, and NaN is a property of a *value*), so this relies on
/// `check_no_nan` having already rejected any NaN in the sort columns before sorting
/// begins.
fn compare_for_sort(this: &Value, other: &Value, mode: ComparisonMode) -> Ordering {
    this.param_partial_cmp(other, mode)
        .expect("NaN values should have been rejected by check_no_nan")
}

/// NaN has no defined position in a sort order, so crush treats it as invalid input
/// rather than silently picking an arbitrary place for it.
fn check_no_nan(rows: &[Row], indices: &[usize]) -> CrushResult<()> {
    for row in rows {
        for idx in indices {
            if let Value::Float(f) = row.cells()[*idx] {
                if f.is_nan() {
                    return command_error("Can't sort a column containing NaN values.");
                }
            }
        }
    }
    Ok(())
}

/// Computes `sort`'s real output type: the same type it was given, unless one of the
/// (unnamed) sort keys names a column that doesn't exist in the input, in which case this
/// returns an argument error pointing at that specific key -- the same check `sort`
/// itself does at runtime via `ColumnVec::find`, just performed before anything runs.
/// There being no input at all, or a known input that isn't stream-shaped, is also
/// predictable ahead of time -- `sort` can never work on it -- so both are a command
/// error rather than being deferred to runtime. Only a genuinely unknown input type
/// (`Unknown`) falls back to `Passthrough`, since nothing can be ruled out yet.
fn sort_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let t = match input {
        PipeInputType::Known(t) => t,
        PipeInputType::Unknown => return Ok(OutputType::Passthrough),
        PipeInputType::None => return command_error("Expected a stream as input."),
    };

    let columns: &[crate::lang::data::table::ColumnType] = match &t {
        ValueType::TableInputStream(columns)
        | ValueType::TableOutputStream(columns)
        | ValueType::Table(columns) => columns,
        _ => return command_error("Expected a stream as input."),
    };

    for arg in arguments {
        if arg.argument_type.is_some() {
            continue;
        }
        if let ArgumentValue::Value(Value::String(name)) = &arg.value {
            if columns.find(name).is_err() {
                return arg.error(format!(
                    "Unknown column `{}`, available columns are {}",
                    name,
                    columns
                        .iter()
                        .map(|c| c.name().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
        }
    }

    Ok(OutputType::Known(t))
}

#[signature(
    stream.sort,
    can_block = true,
    short = "Sort input stream based on one or more of it's columns",
    long = "If any sort column contains a NaN float value, `sort` fails with an error rather \
    than picking an arbitrary position for it, since NaN has no defined ordering relative to \
    other floats.",
    example = "# Show the contents of the current directory, sorted first on type and then on filename",
    example = "files | sort type file",
    output = Passthrough,
    dynamic_output = sort_dynamic_output)]
pub struct Sort {
    #[unnamed()]
    #[description("the columns to sort on. Optional if input only has one column.")]
    field: Vec<String>,
    #[description("reverse the sort order.")]
    #[default(false)]
    reverse: bool,
    #[description("ignore case when sorting textual columns.")]
    #[default(false)]
    case_insensitive: bool,
}

fn sort(mut context: CommandContext) -> CrushResult<()> {
    let mut input = context.input_stream()?;
    let output = context.initialize_output(input.types())?;
    let cfg = Sort::parse(context.remove_arguments(), &context.global_state.printer())?;
    let indices = if cfg.field.is_empty() {
        if input.types().len() == 1 {
            vec![0]
        } else {
            return command_error("Missing comparison key.");
        }
    } else {
        cfg.field
            .iter()
            .map(|f| input.types().find(f))
            .collect::<CrushResult<Vec<_>>>()?
    };

    for idx in &indices {
        if !input.types()[*idx].cell_type.is_comparable() {
            return command_error(format!(
                "Bad comparison key. `{}` is not comparable.",
                input.types()[*idx].name()
            ));
        }
    }

    let mut res: Vec<Row> = Vec::new();

    while let Some(row) = input.next_row()? {
        res.push(row);
    }

    check_no_nan(&res, &indices)?;

    let comparison_mode = match cfg.case_insensitive {
        true => CaseInsensitive,
        false => Regular,
    };

    if cfg.reverse {
        res.sort_by(|a, b| {
            for idx in &indices {
                match compare_for_sort(&b.cells()[*idx], &a.cells()[*idx], comparison_mode) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }
            }
            Ordering::Equal
        });
    } else {
        res.sort_by(|b, a| {
            for idx in &indices {
                match compare_for_sort(&b.cells()[*idx], &a.cells()[*idx], comparison_mode) {
                    Ordering::Equal => {}
                    ordering => return ordering,
                }
            }
            Ordering::Equal
        });
    }

    for row in res {
        output.send(row)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use crate::lang::data::table::ColumnType;
    use std::sync::Arc;

    fn columns() -> Vec<ColumnType> {
        vec![
            ColumnType::new_from_string("file".to_string(), ValueType::String, "".to_string()),
            ColumnType::new_from_string("size".to_string(), ValueType::Integer, "".to_string()),
        ]
    }

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

    #[test]
    fn passes_through_known_input_when_sort_key_is_valid() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::from("file")))];
        match sort_dynamic_output(
            PipeInputType::Known(ValueType::TableInputStream(columns())),
            &args,
        )
        .unwrap()
        {
            OutputType::Known(ValueType::TableInputStream(c)) => {
                assert_eq!(c.len(), 2);
                assert_eq!(c[0].name(), "file");
                assert_eq!(c[1].name(), "size");
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn errors_on_unknown_sort_key() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::from("nonexistent")))];
        assert!(
            sort_dynamic_output(
                PipeInputType::Known(ValueType::TableInputStream(columns())),
                &args,
            )
            .is_err()
        );
    }

    #[test]
    fn ignores_named_arguments_like_reverse() {
        let args = vec![
            unnamed_arg(ArgumentValue::Value(Value::from("file"))),
            named_arg("reverse", ArgumentValue::Value(Value::Bool(true))),
        ];
        assert!(
            sort_dynamic_output(
                PipeInputType::Known(ValueType::TableInputStream(columns())),
                &args,
            )
            .is_ok()
        );
    }

    #[test]
    fn unknown_input_type_is_passthrough() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::from("file")))];
        match sort_dynamic_output(PipeInputType::Unknown, &args).unwrap() {
            OutputType::Passthrough => {}
            other => panic!("expected Passthrough, got {:?}", other),
        }
    }

    #[test]
    fn non_stream_known_input_is_an_error() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::from("file")))];
        assert!(sort_dynamic_output(PipeInputType::Known(ValueType::Integer), &args).is_err());
    }

    #[test]
    fn no_input_at_all_is_an_error() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::from("file")))];
        assert!(sort_dynamic_output(PipeInputType::None, &args).is_err());
    }
}
