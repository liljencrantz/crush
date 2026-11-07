use crate::lang::argument::{ArgumentValue, ValidationArgument};
use crate::lang::ast::source::Source;
use crate::lang::command::{Command, OutputType, PipeInputType};
use crate::lang::data::table::ColumnVec;
use crate::lang::errors::{CrushResult, argument_error, command_error};
use crate::lang::pipe::{Stream, pipe};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::{
    data::table::ColumnType,
    lang::{argument::Argument, data::table::Row, value::Value},
    util::replace::Replace,
};

enum Action {
    Replace(usize),
    Append(String),
}

enum ValueSource {
    Closure(Command),
    Argument(usize),
}

pub struct Config {
    copy: bool,
    columns: Vec<(Action, ValueSource)>,
    source: Source,
}

pub fn run(config: Config, mut input: Stream, context: CommandContext) -> CrushResult<()> {
    let input_type = input.types().to_vec();
    let mut output_type = if config.copy {
        input_type.clone()
    } else {
        vec![]
    };

    for (location, source) in &config.columns {
        let next_type = match source {
            ValueSource::Closure(c) => c
                .output_type(&OutputType::Known(ValueType::TableInputStream(
                    input_type.clone(),
                )))
                .unwrap_or(&ValueType::Any)
                .clone(),
            ValueSource::Argument(idx) => input_type[*idx].cell_type.clone(),
        };
        match location {
            Action::Append(name) => {
                output_type.push(ColumnType::new_from_string(
                    name.clone(),
                    next_type,
                    "a column computed by select.".to_string(),
                ));
            }
            Action::Replace(idx) => {
                let name = output_type[*idx].name().to_string();
                output_type.replace(
                    *idx,
                    ColumnType::new_from_string(name, next_type, "a column computed by select.".to_string()),
                );
            }
        }
    }

    let output_type = output_type.as_slice().deduplicate_names();
    let output = context.initialize_output(&output_type)?;

    while let Some(row) = input.next_row()? {
        let mut next_result = Vec::new();

        if config.copy {
            next_result.append(&mut row.cells().clone());
        }
        for (location, source) in &config.columns {
            let value = match source {
                ValueSource::Closure(closure) => {
                    let arguments: Vec<Argument> = row
                        .cells()
                        .iter()
                        .zip(&input_type)
                        .map(|(cell, cell_type)| {
                            Argument::named(&cell_type.name(), cell.clone(), &config.source)
                        })
                        .collect();
                    let (sender, receiver) = pipe();
                    closure.eval(
                        context
                            .empty()
                            .with_args(arguments, None)
                            .with_output(sender),
                    )?;
                    receiver.recv()?
                }
                ValueSource::Argument(idx) => row.cells()[*idx].clone(),
            };
            match location {
                Action::Append(_) => {
                    next_result.push(value);
                }
                Action::Replace(idx) => {
                    next_result[*idx] = value;
                }
            }
        }
        output.send(Row::new(next_result))?;
    }
    Ok(())
}

/// Computes `select`'s real output type, mirroring `select()`+`run()`'s own parsing and
/// type-computation exactly: a leading unnamed `*` enables `copy` (seeding the output
/// with every input column first); each remaining argument either names an existing
/// input column to copy in (unnamed) or computes a new/replaced column from a closure's
/// own output type (named) -- replacing an existing column of the same name when
/// `copy` is set, appending otherwise. Falls back to `Unknown` whenever the input
/// itself isn't known to be table-shaped yet, or an argument's value isn't resolved to
/// anything concrete yet (e.g. a `{...}` closure literal, which this highlighter can't
/// see the contents of -- see `classify_tokens`'s own notes on nested blocks).
pub fn select_dynamic_output(
    input: PipeInputType,
    arguments: &[ValidationArgument],
) -> CrushResult<OutputType> {
    let input_columns: &[ColumnType] = match &input {
        PipeInputType::Known(
            ValueType::TableInputStream(c) | ValueType::TableOutputStream(c) | ValueType::Table(c),
        ) => c,
        _ => return Ok(OutputType::Unknown),
    };

    let mut arguments = arguments;
    let mut copy = false;
    if let Some(first) = arguments.first() {
        if first.argument_type.is_none() {
            if let ArgumentValue::Value(Value::Glob(g)) = &first.value {
                if g.to_string() == "*" {
                    copy = true;
                    arguments = &arguments[1..];
                }
            }
        }
    }

    let mut output_columns: Vec<ColumnType> = if copy {
        input_columns.to_vec()
    } else {
        Vec::new()
    };

    for arg in arguments {
        match (&arg.argument_type, &arg.value) {
            (Some(name), value) => {
                let next_type = match value {
                    ArgumentValue::Value(Value::Command(cmd)) => cmd
                        .output_type(&OutputType::Known(ValueType::TableInputStream(
                            input_columns.to_vec(),
                        )))
                        .cloned()
                        .unwrap_or(ValueType::Any),
                    ArgumentValue::Unknown | ArgumentValue::Type(_) => ValueType::Any,
                    _ => return arg.error("Invalid argument"),
                };
                match input_columns.find(name) {
                    Ok(idx) if copy => {
                        let col_name = output_columns[idx].name().to_string();
                        output_columns[idx] = ColumnType::new_from_string(
                            col_name,
                            next_type,
                            "a column computed by select.".to_string(),
                        );
                    }
                    _ => output_columns.push(ColumnType::new_from_string(
                        name.clone(),
                        next_type,
                        "a column computed by select.".to_string(),
                    )),
                }
            }
            (None, ArgumentValue::Value(Value::String(name))) => {
                match (copy, input_columns.find(name.as_ref())) {
                    (false, Ok(idx)) => output_columns.push(input_columns[idx].clone()),
                    _ => return arg.error(format!("Unknown column `{}`", name)),
                }
            }
            (None, ArgumentValue::Unknown | ArgumentValue::Type(_)) => return Ok(OutputType::Unknown),
            _ => return arg.error("Invalid argument"),
        }
    }

    let output_columns = output_columns.as_slice().deduplicate_names();
    Ok(OutputType::Known(ValueType::TableInputStream(output_columns)))
}

pub fn select(mut context: CommandContext) -> CrushResult<()> {
    let input = context.input_stream()?;
    let mut copy = false;
    let mut columns = Vec::new();

    if context.arguments.len() == 0 {
        return command_error("No columns selected.");
    }

    let source = context.arguments[0].source.clone();

    if let Value::Glob(g) = &context.arguments[0].value {
        if context.arguments[0].argument_type.is_none() && &g.to_string() == "*" {
            copy = true;
            context.arguments.remove(0);
        } else {
            return argument_error("Invalid argument", &context.arguments[0].source);
        }
    }

    let input_type = input.types();
    for a in &context.arguments {
        match (a.argument_type.as_deref(), a.value.clone()) {
            (Some(name), Value::Command(closure)) => match (copy, input_type.find(name)) {
                (true, Ok(idx)) => {
                    columns.push((Action::Replace(idx), ValueSource::Closure(closure)))
                }

                _ => columns.push((
                    Action::Append(name.to_string()),
                    ValueSource::Closure(closure),
                )),
            },
            (None, Value::String(name)) => match (copy, input_type.find(name.as_ref())) {
                (false, Ok(idx)) => {
                    columns.push((Action::Append(name.to_string()), ValueSource::Argument(idx)))
                }
                _ => {
                    return argument_error(
                        format!("Unknown column `{}`", name).as_str(),
                        &a.source,
                    );
                }
            },
            _ => return argument_error("Invalid argument", &a.source),
        }
    }

    run(
        Config {
            columns,
            copy,
            source,
        },
        input,
        context,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::argument::SwitchStyle;
    use crate::lang::ast::source::{Source, SourceType};
    use crate::util::glob::Glob;
    use crate::lang::state::contexts::CommandContext;
    use signature::signature;
    use std::sync::Arc;

    fn input_columns() -> Vec<ColumnType> {
        vec![
            ColumnType::new("file", ValueType::String, ""),
            ColumnType::new("size", ValueType::Integer, ""),
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

    fn my_cmd(_context: CommandContext) -> CrushResult<()> {
        Ok(())
    }

    #[signature(my_cmd, output = OutputType::Known(ValueType::Duration))]
    struct MyCmdSignature {}

    fn known_input() -> PipeInputType {
        PipeInputType::Known(ValueType::TableInputStream(input_columns()))
    }

    fn column_names(output: OutputType) -> Vec<String> {
        match output {
            OutputType::Known(ValueType::TableInputStream(columns)) => {
                columns.iter().map(|c| c.name().to_string()).collect()
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn copy_all_with_no_extra_columns_passes_through() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::Glob(Glob::new(
            "*",
        ))))];
        let output = select_dynamic_output(known_input(), &args).unwrap();
        assert_eq!(column_names(output), vec!["file", "size"]);
    }

    #[test]
    fn selecting_an_existing_column_by_name_keeps_its_type() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::from("file")))];
        match select_dynamic_output(known_input(), &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(columns)) => {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].name(), "file");
                assert_eq!(columns[0].cell_type, ValueType::String);
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn selecting_an_unknown_column_by_name_is_an_error() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::from("nonexistent")))];
        assert!(select_dynamic_output(known_input(), &args).is_err());
    }

    #[test]
    fn a_named_closure_adds_a_column_of_its_own_output_type() {
        let cmd = MyCmdSignature::create_command();
        let args = vec![named_arg(
            "age",
            ArgumentValue::Value(Value::Command(cmd)),
        )];
        match select_dynamic_output(known_input(), &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(columns)) => {
                assert_eq!(columns.len(), 1);
                assert_eq!(columns[0].name(), "age");
                assert_eq!(columns[0].cell_type, ValueType::Duration);
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn copy_with_a_named_closure_replaces_the_matching_column() {
        let cmd = MyCmdSignature::create_command();
        let args = vec![
            unnamed_arg(ArgumentValue::Value(Value::Glob(Glob::new("*")))),
            named_arg("size", ArgumentValue::Value(Value::Command(cmd))),
        ];
        match select_dynamic_output(known_input(), &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(columns)) => {
                assert_eq!(columns.len(), 2);
                assert_eq!(columns[0].name(), "file");
                assert_eq!(columns[1].name(), "size");
                assert_eq!(columns[1].cell_type, ValueType::Duration);
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn unresolved_closure_falls_back_to_any() {
        let args = vec![named_arg("age", ArgumentValue::Unknown)];
        match select_dynamic_output(known_input(), &args).unwrap() {
            OutputType::Known(ValueType::TableInputStream(columns)) => {
                assert_eq!(columns[0].cell_type, ValueType::Any);
            }
            other => panic!("expected Known(TableInputStream), got {:?}", other),
        }
    }

    #[test]
    fn unknown_input_type_is_unknown() {
        let args = vec![unnamed_arg(ArgumentValue::Value(Value::Glob(Glob::new(
            "*",
        ))))];
        match select_dynamic_output(PipeInputType::Unknown, &args).unwrap() {
            OutputType::Unknown => {}
            other => panic!("expected Unknown, got {:?}", other),
        }
    }

    /// End-to-end regression test for the real `select` command (not a mock): a named
    /// argument (`foo=`) and an unresolved identifier inside its closure body (`$size`,
    /// a row-column binding highlight.rs can't see in scope) must not be highlighted
    /// as errors -- `select`'s own completion_data needs `named: true` for the first,
    /// and highlight.rs's closure leniency covers the second.
    #[test]
    fn select_with_named_closure_argument_is_not_highlighted_as_an_error() {
        let root = crate::lang::state::scope::Scope::create_root();
        crate::builtins::stream::declare(&root).unwrap();

        let mut colors = std::collections::HashMap::new();
        colors.insert("error".to_string(), "<<ERR>>".to_string());

        let rendered = crate::util::highlight::syntax_highlight(
            "select foo={$size}",
            &colors,
            &Some(root),
        )
        .unwrap();

        assert!(
            !rendered.contains("<<ERR>>"),
            "expected no error highlighting, got: {}",
            rendered
        );
    }
}
