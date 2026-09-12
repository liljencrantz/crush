use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushResult;
use crate::lang::errors::command_error;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ComparisonMode::{CaseInsensitive, Regular};
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

#[signature(
    stream.sort,
    can_block = true,
    short = "Sort input stream based on one or more of it's columns",
    long = "If any sort column contains a NaN float value, `sort` fails with an error rather \
    than picking an arbitrary position for it, since NaN has no defined ordering relative to \
    other floats.",
    example = "# Show the contents of the current directory, sorted first on type and then on filename",
    example = "files | sort type file",
    output = Passthrough)]
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
