use crate::lang::command::{ExecutionContext, CrushCommand};
use crate::{
    lib::command_util::find_field_from_str,
    errors::argument_error,
    lang::{
        argument::Argument,
        table::Row,
        value::Value,
    },
    stream::OutputStream,
    replace::Replace,
    lang::table::ColumnType,
    errors::CrushResult,
};
use crate::lib::command_util::find_field;
use crate::stream::{Readable, ValueSender, empty_channel, channels};
use crate::errors::error;
use crate::lang::{r#struct::Struct, table::TableReader};
use crate::lang::command::Closure;
use crate::printer::Printer;
use crate::lang::scope::Scope;

enum Location {
    Replace(usize),
    Append(Box<str>),
}

enum Source {
    Closure(Closure),
    Argument(usize),
}

pub struct Config {
    copy: bool,
    columns: Vec<(Location, Source)>,
}

pub fn run(
    config: Config,
    mut input: Box<dyn Readable>,
    sender: ValueSender,
    env: &Scope,
    printer: &Printer,
) -> CrushResult<()> {
    let input_type = input.types().clone();
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
                            .map(|(cell, cell_type)| Argument::new(cell_type.name.clone(), cell.clone()))
                            .collect();
                        closure.invoke(
                            ExecutionContext {
                                input: empty_channel(),
                                output: sender,
                                arguments,
                                env: env.clone(),
                                printer: printer.clone(),
                            }
                        );
                        receiver.recv()?
                    },
                    Source::Argument(idx) => row.cells()[*idx].clone(),
                };

                match location {
                    Location::Append(name) => {
                        output_type.push(ColumnType::named(name.as_ref(), value.value_type()));
                        first_result.push(value);
                    }
                    Location::Replace(idx) => {
                        output_type.replace(*idx, ColumnType::new(output_type[*idx].name.clone(), value.value_type()));
                        first_result[*idx] = value;
                    }
                }
            }
        }
        Err(_) => return Ok(()),
    }

    let output = sender.initialize(output_type)?;
    output.send(Row::new(first_result))?;

    loop {
        match input.read() {
            Ok(mut row) => {
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
                                .map(|(cell, cell_type)| Argument::new(cell_type.name.clone(), cell.clone()))
                                .collect();
                            let (sender, receiver) = channels();
                            closure.invoke(
                                ExecutionContext {
                                    input: empty_channel(),
                                    output: sender,
                                    arguments,
                                    env: env.clone(),
                                    printer: printer.clone(),
                                }
                            );
                            receiver.recv()?
                        },
                        Source::Argument(idx) => row.cells()[*idx].clone(),
                    };
                    match location {
                        Location::Append(name) => {
                            next_result.push(value);
                        }
                        Location::Replace(idx) => {
                            next_result[*idx] = value;
                        }
                    }
                }
                output.send(Row::new(next_result))?;
            }
            Err(_) => break,
        }
    }
    Ok(())
}

fn perform_for(
    input: Box<dyn Readable>,
    sender: ValueSender,
    mut arguments: Vec<Argument>,
    env: &Scope,
    printer: &Printer,
) -> CrushResult<()> {
    let mut copy = false;
    let mut columns = Vec::new();

    if arguments.len() == 0 {
        return argument_error("No columns selected");
    }

    if let Value::Glob(g) = &arguments[0].value {
        if arguments[0].name.is_none() && g.to_string() == "*" {
            copy = true;
            arguments.remove(0);
        } else {
            return argument_error("Invalid argument");
        }
    }

    let input_type = input.types();
    for a in arguments {
        match (a.name.as_deref(), a.value) {
            (Some(name), Value::Closure(closure)) => {
                match (copy, find_field_from_str(name, input_type)) {
                    (true, Ok(idx)) => columns.push((Location::Replace(idx), Source::Closure(closure))),
                    _ => columns.push((Location::Append(Box::from(name)), Source::Closure(closure))),
                }
            }
            (None, Value::Text(name)) => {
                match (copy, find_field_from_str(name.as_ref(), input_type)) {
                    (false, Ok(idx)) => columns.push((Location::Append(name), Source::Argument(idx))),
                    _ => return argument_error(format!("Unknown field {}", name.as_ref()).as_str()),
                }
            }
            _ => return argument_error("Invalid argument"),
        }
    }

    run(Config { columns, copy }, input, sender, env, printer)
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(r) => perform_for(r, context.output, context.arguments, &context.env, &context.printer),
        _ => error("Expected a stream"),
    }
}
