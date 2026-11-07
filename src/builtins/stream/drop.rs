use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::command::OutputType::Unknown;
use crate::lang::command::{OutputType, PipeInputType};
use crate::lang::data::table::{ColumnType, ColumnVec, Row};
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::{Value, ValueType};
use signature::signature;
use std::collections::HashSet;

/// Computes `drop`'s real output type: every input column except the ones named --
/// the exact inverse of `select`. Falls back to `Unknown` when the input isn't known to
/// be table-shaped yet, or when a column name isn't resolved to a literal yet.
fn drop_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let input_columns: &[ColumnType] = match &input {
        PipeInputType::Known(
            ValueType::TableInputStream(c) | ValueType::TableOutputStream(c) | ValueType::Table(c),
        ) => c,
        _ => return Ok(OutputType::Unknown),
    };

    let mut drop_indices = HashSet::new();
    for arg in arguments {
        match &arg.value {
            ArgumentValue::Value(Value::String(s)) => match input_columns.find(s) {
                Ok(idx) => {
                    drop_indices.insert(idx);
                }
                Err(_) => return arg.error(format!("Unknown column `{}`", s)),
            },
            ArgumentValue::Unknown | ArgumentValue::Type(_) => return Ok(OutputType::Unknown),
            _ => return arg.error("Invalid argument"),
        }
    }

    let output_columns: Vec<ColumnType> = input_columns
        .iter()
        .enumerate()
        .filter(|(idx, _)| !drop_indices.contains(idx))
        .map(|(_, c)| c.clone())
        .collect();
    Ok(OutputType::Known(ValueType::TableInputStream(output_columns)))
}

#[signature(
    stream.drop,
    can_block = true,
    short = "Drop specified columns from input stream, copy content of remaining columns into output",
    long = "This command is does the opposite of the select command. It copies all columns except the ones specified from input to output.",
    example = "# Drop memory usage columns from output of ps",
    example = "host:procs | drop vms rss",
    output = Unknown,
    dynamic_output = drop_dynamic_output,
)]
pub struct Drop {
    #[unnamed()]
    #[description("the columns to remove from the stream.")]
    drop: Vec<String>,
}

fn drop(mut context: CommandContext) -> CrushResult<()> {
    let cfg = Drop::parse(
        context.remove_arguments().clone(),
        &context.global_state.printer(),
    )?;
    let mut input = context.input_stream()?;
    let t = input.types();
    let drop = cfg
        .drop
        .iter()
        .map(|f| t.find(f))
        .collect::<CrushResult<HashSet<usize>>>()?;
    let inc: Vec<bool> = (0..t.len())
        .into_iter()
        .map(|idx| drop.contains(&idx))
        .collect();
    let mut it = inc.iter();
    let output = context.initialize_output(
        &t.to_vec()
            .drain(..)
            .filter(|_| !*(it.next().unwrap()))
            .collect::<Vec<_>>(),
    )?;
    while let Some(row) = input.next_row()? {
        let mut row = Vec::from(row);
        let mut it = inc.iter();
        output.send(Row::new(
            row.drain(..).filter(|_| !*(it.next().unwrap())).collect(),
        ))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use std::sync::Arc;

    fn columns() -> Vec<ColumnType> {
        vec![
            ColumnType::new("vms", ValueType::Integer, ""),
            ColumnType::new("rss", ValueType::Integer, ""),
            ColumnType::new("name", ValueType::String, ""),
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

    fn known() -> PipeInputType {
        PipeInputType::Known(ValueType::TableInputStream(columns()))
    }

    #[test]
    fn dropped_columns_are_removed() {
        let args = [
            unnamed_arg(ArgumentValue::Value(Value::from("vms"))),
            unnamed_arg(ArgumentValue::Value(Value::from("rss"))),
        ];
        match drop_dynamic_output(known(), &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(c)) => {
                assert_eq!(c.len(), 1);
                assert_eq!(c[0].name(), "name");
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn no_arguments_keeps_every_column() {
        match drop_dynamic_output(known(), &[]).unwrap() {
            OutputType::Known(ValueType::TableInputStream(c)) => assert_eq!(c.len(), 3),
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn unknown_column_is_an_error() {
        let args = [unnamed_arg(ArgumentValue::Value(Value::from("nonexistent")))];
        assert!(drop_dynamic_output(known(), &args).is_err());
    }

    #[test]
    fn unresolved_column_falls_back_to_unknown() {
        let args = [unnamed_arg(ArgumentValue::Unknown)];
        match drop_dynamic_output(known(), &args).unwrap() {
            OutputType::Unknown => {}
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn unknown_input_type_is_unknown() {
        match drop_dynamic_output(PipeInputType::Unknown, &[]).unwrap() {
            OutputType::Unknown => {}
            other => panic!("expected Unknown, got {:?}", other),
        }
    }
}
