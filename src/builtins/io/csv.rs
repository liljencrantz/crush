use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::command::{OutputType, PipeInputType};
use crate::lang::state::contexts::CommandContext;
use crate::{
    lang::errors::CrushError,
    lang::{data::table::Row, value::Value},
};
use std::{io::BufReader, io::prelude::*};

use crate::lang::data::table::ColumnType;
use crate::lang::errors::{CrushResult, error};

use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::binary_input::ToReader;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::value::ValueType;
use signature::signature;

/// The names of `From`'s own fields other than `columns` -- every other named argument is
/// a caller-defined column, collected into `columns` by the `#[named()]` fallback (see
/// the `From` struct below).
const NON_COLUMN_ARGUMENTS: [&str; 3] = ["separator", "head", "trim"];

/// Computes `$csv.from`'s real output type: a `table_input_stream` whose columns are
/// exactly the caller's `name=type` arguments (e.g. `name=$string age=$integer`), in the
/// order given -- the static `output` a `#[signature(...)]` command would otherwise
/// declare can't express this, since the columns depend entirely on how the command was
/// called. Falls back to `Unknown` if any column's type isn't yet resolvable (e.g. it
/// came from a command substitution rather than a literal type), and rejects a column
/// argument whose value could never be a type at all.
fn from_dynamic_output(
    _input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let mut columns = Vec::new();
    for arg in arguments {
        let Some(name) = &arg.argument_type else {
            continue;
        };
        if NON_COLUMN_ARGUMENTS.contains(&name.as_str()) {
            continue;
        }
        match &arg.value {
            ArgumentValue::Value(Value::Type(t)) => {
                columns.push(ColumnType::new_from_string(
                    name.clone(),
                    t.clone(),
                    "a column defined by the caller.".to_string(),
                ));
            }
            ArgumentValue::Value(_) => {
                return arg.error(format!(
                    "Column `{}` must be given a type, e.g. `{}=$string`",
                    name, name
                ));
            }
            ArgumentValue::Unknown | ArgumentValue::Type(_) => {
                return Ok(OutputType::Unknown);
            }
        }
    }
    if columns.is_empty() {
        Ok(OutputType::Unknown)
    } else {
        Ok(OutputType::Known(ValueType::TableInputStream(columns)))
    }
}

#[signature(
    io.csv.from,
    example = "$csv.from separator=\",\" head=1 name=$string age=$integer nick=$string",
    short = "Parse specified files as CSV files.",
    dynamic_output = from_dynamic_output,
)]
struct From {
    #[unnamed()]
    #[description(
        "source. If unspecified, will read from input, which must be a binary or binary_stream."
    )]
    files: Vec<BinaryInput>,
    #[named()]
    #[description("name and type of all columns.")]
    columns: OrderedStringMap<ValueType>,
    #[description("column separator.")]
    #[default(',')]
    separator: char,
    #[default(0usize)]
    #[description("skip this many lines of input from the beginning.")]
    head: usize,
    #[description("trim this character from start and end of every value.")]
    trim: Option<char>,
}

fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.remove_arguments(), &context.global_state.printer())?;
    let columns = cfg
        .columns
        .iter()
        .map(|(k, v)| ColumnType::new_from_string(k.clone(), v.clone(), "a column defined by the caller.".to_string()))
        .collect::<Vec<_>>();
    let output = context.initialize_output(&columns)?;
    let command_handle = context.command_handle().clone();

    let mut reader = BufReader::new(cfg.files.to_reader(context.input, &command_handle)?);

    let separator = cfg.separator;
    let trim = cfg.trim;
    let skip = cfg.head;

    let mut line = String::new();
    let mut skipped = 0usize;
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        if line.is_empty() {
            break;
        }
        if skipped < skip {
            skipped += 1;
            continue;
        }
        let line_without_newline = if line.ends_with('\n') {
            &line[0..line.len() - 1]
        } else {
            &line
        };

        let mut split: Vec<&str> = line_without_newline
            .split(separator)
            .map(|s| trim.map(|c| s.trim_matches(c)).unwrap_or(s))
            .collect();

        if split.len() != columns.len() {
            return error("Wrong number of columns in CSV file.");
        }

        if let Some(trim) = trim {
            split = split.iter().map(|s| s.trim_matches(trim)).collect();
        }

        match split
            .iter()
            .zip(columns.iter())
            .map(|(s, t)| t.cell_type.parse(*s))
            .collect::<Result<Vec<Value>, CrushError>>()
        {
            Ok(cells) => {
                let _ = output.send(Row::new(cells));
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use std::sync::Arc;

    fn column_arg(name: &str, value: ArgumentValue) -> ValidationArgument {
        ValidationArgument {
            argument_type: Some(name.to_string()),
            switch_style: SwitchStyle::None,
            value,
            source: Source::new(SourceType::Input, Arc::from("")),
        }
    }

    #[test]
    fn computes_columns_from_named_type_arguments() {
        let args = vec![
            column_arg("separator", ArgumentValue::Value(Value::from(','))),
            column_arg("name", ArgumentValue::Value(Value::Type(ValueType::String))),
            column_arg("age", ArgumentValue::Value(Value::Type(ValueType::Integer))),
        ];
        match from_dynamic_output(PipeInputType::None, &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(columns)) => {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].name(), "name");
                assert_eq!(columns[0].cell_type, ValueType::String);
                assert_eq!(columns[1].name(), "age");
                assert_eq!(columns[1].cell_type, ValueType::Integer);
            }
            other => panic!("expected a known table_input_stream, got {:?}", other),
        }
    }

    #[test]
    fn no_column_arguments_is_unknown() {
        let args = vec![column_arg(
            "separator",
            ArgumentValue::Value(Value::from(',')),
        )];
        match from_dynamic_output(PipeInputType::None, &args).unwrap() {
            OutputType::Unknown => {}
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn unresolved_column_type_falls_back_to_unknown() {
        let args = vec![column_arg("name", ArgumentValue::Type(ValueType::String))];
        match from_dynamic_output(PipeInputType::None, &args).unwrap() {
            OutputType::Unknown => {}
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    #[test]
    fn non_type_column_value_is_an_error() {
        let args = vec![column_arg("name", ArgumentValue::Value(Value::from("oops")))];
        assert!(from_dynamic_output(PipeInputType::None, &args).is_err());
    }
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "csv",
        "CSV I/O",
        None,
        Box::new(move |env| {
            From::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
