use crate::lang::command::Command;
use crate::{
    lang::errors::argument_error,
    lang::{
        argument::Argument,
        table::Row,
        value::Value,
    },
    util::replace::Replace,
    lang::table::ColumnType,
    lang::errors::CrushResult,
};
use crate::lang::stream::{Readable, empty_channel, channels};
use crate::lang::errors::error;
use crate::lang::table::ColumnVec;
use crate::lang::execution_context::ExecutionContext;

enum Location {
    Replace(usize),
    Append(String),
}

enum Source {
    Closure(Command),
    Argument(usize),
}

pub struct Config {
    copy: bool,
    columns: Vec<(Location, Source)>,
}

pub fn run(
    config: Config,
    mut input: Box<dyn Readable>,
    context: ExecutionContext,
) -> CrushResult<()> {
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
                        let (sender, receiver) = channels();
                        let arguments: Vec<Argument> = row
                            .cells()
                            .iter()
                            .zip(&input_type)
                            .map(|(cell, cell_type)| Argument::named(cell_type.name.as_ref(), cell.clone()))
                            .collect();
                        closure.invoke(
                            ExecutionContext {
                                input: empty_channel(),
                                output: sender,
                                arguments,
                                env: context.env.clone(),
                                this: None,
                                printer: context.printer.clone(),
                            }
                        )?;
                        receiver.recv()?
                    }
                    Source::Argument(idx) => row.cells()[*idx].clone(),
                };

                match location {
                    Location::Append(name) => {
                        output_type.push(ColumnType::new(name.as_ref(), value.value_type()));
                        first_result.push(value);
                    }
                    Location::Replace(idx) => {
                        output_type.replace(*idx, ColumnType::new(output_type[*idx].name.as_ref(), value.value_type()));
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
                        .map(|(cell, cell_type)| Argument::named(&cell_type.name, cell.clone()))
                        .collect();
                    let (sender, receiver) = channels();
                    closure.invoke(
                        ExecutionContext {
                            input: empty_channel(),
                            output: sender,
                            arguments,
                            env: context.env.clone(),
                            this: None,
                            printer: context.printer.clone(),
                        }
                    )?;
                    receiver.recv()?
                }
                Source::Argument(idx) => row.cells()[*idx].clone(),
            };
            match location {
                Location::Append(_) => {
                    next_result.push(value);
                }
                Location::Replace(idx) => {
                    next_result[*idx] = value;
                }
            }
        }
        output.send(Row::new(next_result))?;
    }
    Ok(())
}


pub fn select(mut context: ExecutionContext) -> CrushResult<()> {
    match context.input.clone().recv()?.readable() {
        Some(input) => {
            let mut copy = false;
            let mut columns = Vec::new();

            if context.arguments.len() == 0 {
                return argument_error("No columns selected");
            }

            if let Value::Glob(g) = &context.arguments[0].value {
                if context.arguments[0].argument_type.is_none() && &g.to_string() == "%" {
                    copy = true;
                    context.arguments.remove(0);
                } else {
                    return argument_error("Invalid argument");
                }
            }

            let input_type = input.types();
            for a in &context.arguments {
                match (a.argument_type.as_deref(), a.value.clone()) {
                    (Some(name), Value::Command(closure)) => {
                        match (copy, input_type.find_str(name)) {
                            (true, Ok(idx)) => columns.push((Location::Replace(idx), Source::Closure(closure))),
                            _ => columns.push((Location::Append(name.to_string()), Source::Closure(closure))),
                        }
                    }
                    (None, Value::Field(name)) => {
                        if name.len() != 1 {
                            return argument_error("Invalid field");
                        }
                        match (copy, input_type.find_str(name[0].as_ref())) {
                            (false, Ok(idx)) => columns.push((Location::Append(name[0].clone()), Source::Argument(idx))),
                            _ => return argument_error(format!("Unknown field {}", name[0]).as_str()),
                        }
                    }
                    _ => return argument_error("Invalid argument"),
                }
            }

            run(Config { columns, copy }, input, context)
        }
        _ => error("Expected a stream"),
    }
}
