use crate::{
    data::Argument,
    data::Cell,
    errors::JobError,
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::{JobOutput, ColumnType, CellType, Row};
use crate::errors::{argument_error, JobResult, error};
use crate::closure::ClosureDefinition;
use crate::commands::{CompileContext, JobJoinHandle};
use crate::stream::{empty_stream, InputStream, OutputStream, UninitializedOutputStream, streams, UninitializedInputStream, unlimited_streams};
use crate::stream_printer::spawn_print_thread;
use either::Either;
use crate::commands::command_util::find_field;
use std::thread;
use std::thread::JoinHandle;
use crate::replace::Replace;
use std::sync::mpsc::channel;
use crate::commands::join::guess_tables;

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
                CellType::Output(sub_types) | CellType::Rows(sub_types) => Some(idx),
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
    let table_idx = match (argument.len() % 2, argument[0].name.is_none()) {
        (0, false) => guess_table(input_type)?,
        (1, true) => panic!("NOT IMPLEMENTED"),
        _ => return Err(argument_error("Could not find table to aggregate")),
    };

    if let CellType::Output(sub_type) = &input_type[table_idx].cell_type {
        let output_definition = argument
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
        //          println!("WEEEE {:?}", &output_definition);
        Ok(Config {
            table_idx,
            output_definition,
        })
    } else {
        Err(argument_error("No table to aggregate on found"))
    }
    /*
        [
            (
                "ss",
                1,
             ClosureDefinition {
                 job_definitions: [JobDefinition {
                     commands: [CallDefinition {
                         name: ["sum"], arguments: [
                             BaseArgument {
                                 name: None,
                                 cell: Field(["size"]) }] }] }
                 ],
                 env: Some(Env { namespace: Mutex { data: Namespace { parent: None,
                     data: {"aggr": Command(Command),
                         "sum": Command(Command),
                         "reverse": Command(Command),
                         "enumerate": Command(Command),
                         "csv": Command(Command),
                         "set": Command(Command),
                         "for": Command(Command), "where": Command(Command), "tail": Command(Command),
                         "sort": Command(Command), "echo": Command(Command), "cd": Command(Command),
                         "let": Command(Command), "count": Command(Command), "cat": Command(Command),
                         "cast": Command(Command), "unset": Command(Command), "head": Command(Command),
                         "select": Command(Command), "join": Command(Command), "pwd": Command(Command),
                         "lines": Command(Command), "ls": Command(Command), "find": Command(Command), "group": Command(Command), "ps": Command(Command)} } } }) }),
            ("cnt", 0,
             ClosureDefinition {
                 job_definitions: [
                     JobDefinition {
                         commands: [
                             CallDefinition { name: ["count"], arguments: [] }] }],
                 env: Some(Env { namespace: Mutex { data: Namespace { parent: None,
                     data: {
                         "aggr": Command(Command),
                         "sum": Command(Command), "reverse": Command(Command), "enumerate": Command(Command),
                         "csv": Command(Command), "set": Command(Command), "for": Command(Command),
                         "where": Command(Command), "tail": Command(Command), "sort": Command(Command),
                         "echo": Command(Command), "cd": Command(Command), "let": Command(Command),
                         "count": Command(Command), "cat": Command(Command), "cast": Command(Command), "unset": Command(Command), "head": Command(Command),
                         "select": Command(Command), "join": Command(Command), "pwd": Command(Command), "lines": Command(Command), "ls": Command(Command),
                         "find": Command(Command), "group": Command(Command), "ps": Command(Command)} } } }) }
        )
        ]
    */
}


fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

fn handle(h: Result<JoinHandle<JobResult<()>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}

pub fn run(config: Config, printer: &Printer, env: &Env, input: InputStream, uninitialized_output: UninitializedOutputStream) -> JobResult<()> {
//    println!("Run aggregator");

    let (writer_output, writer_input) = channel::<Row>();
    let mut output_names = input.get_type().iter().map(|t| t.name.clone()).collect::<Vec<Option<Box<str>>>>();
    output_names.remove(config.table_idx);
    for (name, _, _) in &config.output_definition {
        output_names.push(Some(name.clone().into_boxed_str()));
    }
    let writer_handle = handle(build("aggr-writer".to_string()).spawn(
        move || {
            println!("Created writer thread");
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
        }));

    loop {
        match input.recv() {
            Ok(mut row) => {
                let table_cell = row.cells.remove(config.table_idx);
                if let Cell::JobOutput(job_output) = table_cell {
                    let mut outputs: Vec<OutputStream> = Vec::new();
                    let mut uninitialized_inputs: Vec<UninitializedInputStream> = Vec::new();

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
                        handle(build("aggr-aggregator".to_string()).spawn(
                            move || {
                                cc.spawn_and_execute(CompileContext {
                                    input: first_input,
                                    output: last_output,
                                    arguments: vec![],
                                    env: local_env,
                                    printer: local_printer,
                                });
                                Ok(())
                            }));
                    }

                    let my_writer_output = writer_output.clone();
                    let collector_handle = handle(build("aggr-collector".to_string()).spawn(
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
                                    my_writer_output.send(partial_row);
                                }
                                Err(_) => {}
                            }
                            Ok(())
                        }));

                    rest_output.send(row)?;

                    loop {
                        match job_output.stream.recv() {
                            Ok(mut inner_row) => {
                                for (stream_idx, (_, column_idx, _)) in config.output_definition.iter().enumerate() {
                                    outputs[stream_idx].send(Row { cells: vec![inner_row.cells.replace(*column_idx, Cell::Integer(0))] })?;
                                }
                            }
                            Err(_) => break,
                        }
                    }
                    drop(rest_output);
                    drop(outputs);
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
