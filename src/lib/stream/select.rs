use crate::lang::command::Command;
use crate::lang::errors::error;
use crate::lang::execution_context::CommandContext;
use crate::lang::pipe::{pipe, Stream};
use crate::lang::data::table::ColumnVec;
use crate::{
    lang::errors::argument_error_legacy,
    lang::errors::CrushResult,
    data::table::ColumnType,
    lang::{argument::Argument, data::table::Row, value::Value},
    util::replace::Replace,
};
use crate::lang::ast::location::Location;

enum Action {
    Replace(usize),
    Append(String),
}

enum Source {
    Closure(Command),
    Argument(usize),
}

pub struct Config {
    copy: bool,
    columns: Vec<(Action, Source)>,
    location: Location,
}

pub fn run(config: Config, mut input: Stream, context: CommandContext) -> CrushResult<()> {
    let input_type = input.types().to_vec();
    let mut output_type = if config.copy {
        input_type.clone()
    } else {
        Vec::new()
    };

    let mut first_result = Vec::new();

    match input.read() {
        Ok(row) => {
            if config.copy {
                first_result.append(&mut row.cells().clone());
            }
            for (location, source) in &config.columns {
                let value = match source {
                    Source::Closure(closure) => {
                        let (sender, receiver) = pipe();
                        let arguments: Vec<Argument> = row
                            .cells()
                            .iter()
                            .zip(&input_type)
                            .map(|(cell, cell_type)| {
                                Argument::named(cell_type.name.as_ref(), cell.clone(), config.location)
                            })
                            .collect();
                        closure.eval(context.empty().with_args(arguments, None).with_output(sender))?;
                        receiver.recv()?
                    }
                    Source::Argument(idx) => row.cells()[*idx].clone(),
                };

                match location {
                    Action::Append(name) => {
                        output_type.push(ColumnType::new(name.as_ref(), value.value_type()));
                        first_result.push(value);
                    }
                    Action::Replace(idx) => {
                        output_type.replace(
                            *idx,
                            ColumnType::new(output_type[*idx].name.as_ref(), value.value_type()),
                        );
                        first_result[*idx] = value;
                    }
                }
            }
        }
        Err(_) => return Ok(()),
    }

    let output = context.output.initialize(output_type)?;
    output.send(Row::new(first_result))?;

    while let Ok(row) = input.read() {
        let mut next_result = Vec::new();

        if config.copy {
            next_result.append(&mut row.cells().clone());
        }
        for (location, source) in &config.columns {
            let value = match source {
                Source::Closure(closure) => {
                    let arguments: Vec<Argument> = row
                        .cells()
                        .iter()
                        .zip(&input_type)
                        .map(|(cell, cell_type)| Argument::named(&cell_type.name, cell.clone(), config.location))
                        .collect();
                    let (sender, receiver) = pipe();
                    closure.eval(context.empty().with_args(arguments, None).with_output(sender))?;
                    receiver.recv()?
                }
                Source::Argument(idx) => row.cells()[*idx].clone(),
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

pub fn select(mut context: CommandContext) -> CrushResult<()> {
    match context.input.clone().recv()?.stream()? {
        Some(input) => {
            let mut copy = false;
            let mut columns = Vec::new();

            if context.arguments.len() == 0 {
                return argument_error_legacy("No columns selected");
            }

            if let Value::Glob(g) = &context.arguments[0].value {
                if context.arguments[0].argument_type.is_none() && &g.to_string() == "%" {
                    copy = true;
                    context.arguments.remove(0);
                } else {
                    return argument_error_legacy("Invalid argument");
                }
            }

            let mut location = context.arguments[0].location;
            let input_type = input.types();
            for a in &context.arguments {
                location = location.union(a.location);
                match (a.argument_type.as_deref(), a.value.clone()) {
                    (Some(name), Value::Command(closure)) => {
                        match (copy, input_type.find(name)) {
                            (true, Ok(idx)) => {
                                columns.push((Action::Replace(idx), Source::Closure(closure)))
                            }
                            _ => columns.push((
                                Action::Append(name.to_string()),
                                Source::Closure(closure),
                            )),
                        }
                    }
                    (None, Value::Symbol(name)) => {
                        match (copy, input_type.find(name.as_ref())) {
                            (false, Ok(idx)) => columns
                                .push((Action::Append(name.clone()), Source::Argument(idx))),
                            _ => {
                                return argument_error_legacy(
                                    format!("Unknown column {}", name).as_str(),
                                );
                            }
                        }
                    }
                    _ => return argument_error_legacy("Invalid argument"),
                }
            }

            run(Config { columns, copy, location }, input, context)
        }
        _ => error("Expected a stream"),
    }
}
