use crate::{
    data::Argument,
    data::Cell,
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::{JobOutput, ColumnType, CellType, Row};
use crate::errors::{argument_error, JobResult, error};
use crate::closure::ClosureDefinition;
use crate::commands::{CompileContext, JobJoinHandle};
use crate::stream::{InputStream, OutputStream, UninitializedOutputStream, streams, UninitializedInputStream};
use crate::commands::command_util::find_field;
use crate::replace::Replace;
use std::sync::mpsc::{channel, Receiver, Sender};
use crate::thread_util::{handle, build};

pub struct Config {
    table_idx: usize,
    output_definition: Vec<(String, usize, ClosureDefinition)>,
}

pub fn guess_table(input_type: &Vec<ColumnType>) -> JobResult<usize> {
    let tables: Vec<usize> = input_type
        .iter()
        .enumerate()
        .flat_map(|(idx, t)| {
            match &t.cell_type {
                CellType::Output(_) | CellType::Rows(_) => Some(idx),
                _ => None,
            }
        }).collect();
    if tables.len() == 1 {
        Ok(tables[0])
    } else {
        Err(argument_error(format!("Could not guess tables to join, expected one table, found {}", tables.len()).as_str()))
    }
}


pub fn parse(input_type: &Vec<ColumnType>, argument: Vec<Argument>) -> JobResult<Config> {
    if argument.len() < 2 {
        return Err(argument_error("Expected at least two paramaters"));
    }
    let (table_idx, aggregations) = match (argument.len() % 2, argument[0].name.is_none(), &argument[0].cell) {
        (0, false, _) => (guess_table(input_type)?, &argument[..]),
        (1, true, Cell::Field(f)) => (find_field(&f, input_type)?, &argument[1..]),
        _ => return Err(argument_error("Could not find table to aggregate")),
    };

    if let CellType::Output(sub_type) = &input_type[table_idx].cell_type {
        let output_definition = aggregations
            .chunks(2)
            .into_iter()
            .map(|args| {
                let spec = &args[0];
                let clos = &args[1];
                match (&spec.name, &spec.cell, &clos.cell) {
                    (Some(name), Cell::Field(f), Cell::ClosureDefinition(c)) =>
                        Ok((
                            name.to_string(),
                            find_field(&f, sub_type)?,
                            c.clone()
                        )),
                    _ => Err(error("Invalid aggragation spec")),
                }
            })
            .collect::<JobResult<Vec<(String, usize, ClosureDefinition)>>>()?;
        Ok(Config {
            table_idx,
            output_definition,
        })
    } else {
        Err(argument_error("No table to aggregate on found"))
    }
}


fn create_writer(
    uninitialized_output: UninitializedOutputStream,
    mut output_names: Vec<Option<Box<str>>>,
    writer_input: Receiver<Row>) ->
    JobJoinHandle {
    handle(build("aggr-writer".to_string()).spawn(
        move || {
            let output = match writer_input.recv() {
                Ok(row) => {
                    let tmp = uninitialized_output.initialize(
                        row.cells
                            .iter()
                            .enumerate()
                            .map(|(idx, cell)| ColumnType { name: output_names[idx].take(), cell_type: cell.cell_type() })
                            .collect()
                    )?;
                    tmp.send(row);
                    tmp
                }
                Err(_) => return Err(error("No output")),
            };

            loop {
                match writer_input.recv() {
                    Ok(row) => {
                        output.send(row);
                    }
                    Err(_) => break,
                }
            }
            Ok(())
        }))
}

pub fn create_collector(
    rest_input: InputStream,
    uninitialized_inputs: Vec<UninitializedInputStream>,
    writer_output: Sender<Row>) -> JobJoinHandle {
    handle(build("aggr-collector".to_string()).spawn(
        move || {
            match rest_input.recv() {
                Ok(mut partial_row) => {
                    for ui in uninitialized_inputs {
                        let i = ui.initialize()?;
                        match i.recv() {
                            Ok(mut r) => {
                                partial_row.cells.push(std::mem::replace(&mut r.cells[0], Cell::Integer(0)));
                            }
                            Err(_) => return Err(error("Missing value")),
                        }
                    }
                    writer_output.send(partial_row);
                }
                Err(_) => {}
            }
            Ok(())
        }))
}

pub fn pump_table(
    job_output: &JobOutput,
    outputs: Vec<OutputStream>,
    output_definition: &Vec<(String, usize, ClosureDefinition)>) -> JobResult<()>{

    let stream_to_column_mapping = output_definition.iter().map(|(_, off, _)| *off).collect::<Vec<usize>>();

    loop {
        match job_output.stream.recv() {
            Ok(mut inner_row) => {
                for stream_idx in 0..stream_to_column_mapping.len() {
                    outputs[stream_idx].send(Row { cells: vec![inner_row.cells.replace(stream_to_column_mapping[stream_idx], Cell::Integer(0))] })?;
                }
            }
            Err(_) => break,
        }
    }
    Ok(())
}

pub fn run(config: Config, printer: &Printer, env: &Env, input: InputStream, uninitialized_output: UninitializedOutputStream) -> JobResult<()> {
    let (writer_output, writer_input) = channel::<Row>();

    let mut output_names = input.get_type().iter().map(|t| t.name.clone()).collect::<Vec<Option<Box<str>>>>();
    output_names.remove(config.table_idx);
    for (name, _, _) in &config.output_definition {
        output_names.push(Some(name.clone().into_boxed_str()));
    }

    let writer_handle = create_writer(uninitialized_output, output_names, writer_input);

    loop {
        match input.recv() {
            Ok(mut row) => {
                let table_cell = row.cells.remove(config.table_idx);
                if let Cell::JobOutput(job_output) = table_cell {
                    let mut outputs: Vec<OutputStream> = Vec::new();
                    let mut uninitialized_inputs: Vec<UninitializedInputStream> = Vec::new();
                    let mut aggregator_handles: Vec<JobJoinHandle> = Vec::new();

                    let (uninit_rest_output, uninit_rest_input) = streams();
                    let mut rest_output_type = input.get_type().clone();
                    rest_output_type.remove(config.table_idx);
                    let rest_output = uninit_rest_output.initialize(rest_output_type)?;
                    let rest_input = uninit_rest_input.initialize()?;

                    for (name, idx, c) in config.output_definition.iter() {
                        let (first_output, first_input) = streams();
                        let (last_output, last_input) = streams();
                        outputs.push(first_output.initialize(
                            vec![
                                ColumnType::named(name, job_output.stream.get_type()[*idx].cell_type.clone())
                            ]
                        )?);
                        uninitialized_inputs.push(last_input);

                        let local_printer = printer.clone();
                        let local_env = env.clone();
                        let cc = c.clone();
                        aggregator_handles.push(handle(build("aggr-aggregator".to_string()).spawn(
                            move || {
                                cc.spawn_and_execute(CompileContext {
                                    input: first_input,
                                    output: last_output,
                                    arguments: vec![],
                                    env: local_env,
                                    printer: local_printer,
                                });
                                Ok(())
                            })));
                    }

                    let collector_handle = create_collector(
                        rest_input,
                        uninitialized_inputs,
                        writer_output.clone());

                    rest_output.send(row)?;
                    drop(rest_output);

                    pump_table(&job_output, outputs, &config.output_definition)?;


                    for h in aggregator_handles {
                        h.join(printer);
                    }
                    collector_handle.join(printer);
                }
            }
            Err(_) => { break; }
        }
    }
    drop(writer_output);
    writer_handle.join(printer);
    Ok(())
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;

    let config = parse(input.get_type(), context.arguments)?;
    run(config, &context.printer, &context.env, input, context.output)
}
