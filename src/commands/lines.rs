use crate::commands::CompileContext;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std::thread;
use std::path::Path;
use lazy_static::lazy_static;
use crate::{
    commands::command_util::find_field_from_str,
    errors::{argument_error},
    data::{
        Argument,
        Row,
        ColumnType,
        CellType,
        JobOutput,
        Cell,
    },
    stream::{OutputStream, InputStream, unlimited_streams},
};
use either::Either;
use crate::errors::JobResult;
use crate::commands::command_util::find_field;

lazy_static! {
    static ref sub_type: Vec<ColumnType> = {
        vec![ColumnType::named("line", CellType::Text)]
    };
}

fn handle(file: Box<Path>, output: &OutputStream) -> JobResult<()> {
    let (uninit_output_stream, input_stream) = unlimited_streams();
    let output_stream = uninit_output_stream.initialize(sub_type.clone())?;
    let out_row = Row {
        cells: vec![
            Cell::File(file.clone()),
            Cell::JobOutput(JobOutput {
                stream: input_stream.initialize()?,
            }),
        ],
    };

    output.send(out_row)?;
    thread::spawn(move || {
        let fff = File::open(file).unwrap();
        let mut reader = BufReader::new(&fff);
        let mut line = String::new();
        loop {
            reader.read_line(&mut line);
            if line.is_empty() {
                break;
            }
            output_stream.send(Row { cells: vec![Cell::Text(line[0..line.len() - 1].to_string().into_boxed_str())] });
            line.clear();
        }
    });
    return Ok(());
}


pub struct Config {
    files: Either<(usize, InputStream), Vec<Box<Path>>>,
}

fn parse(arguments: Vec<Argument>, input: InputStream) -> JobResult<Config> {
    if input.get_type().len() == 0 {
        let mut files: Vec<Box<Path>> = Vec::new();
        for arg in &arguments {
            arg.cell.file_expand(&mut files)?;
        }
        Ok(Config { files: Either::Right(files) })
    } else {
        if arguments.len() != 1 {
            return Err(argument_error("Expected one argument: column spec"));
        }
        match &arguments[0].cell {
            Cell::Text(s) => {
                Ok(Config {
                    files: Either::Left((find_field_from_str(&s, input.get_type())?, input)),
                })
            }
            Cell::Field(s) => {
                Ok(Config {
                    files: Either::Left((find_field(s, input.get_type())?, input)),
                })
            }
            _ => return Err(argument_error("Expected column of type Field")),
        }
    }
}

pub fn run(config: Config, output: OutputStream) -> JobResult<()> {
    match config.files {
        Either::Left((idx, input)) => {
            loop {
                match input.recv() {
                    Ok(row) => {
                        let mut files: Vec<Box<Path>> = Vec::new();
                        row.cells[idx].file_expand(&mut files)?;
                        for file in files {
                            handle(file, &output)?;
                        }
                    },
                    Err(_) => break,
                }
            }

        },
        Either::Right(files) => {
            for file in files {
                handle(file, &output)?;
            }
        },
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(
        vec![
            ColumnType::named("file", CellType::File),
            ColumnType::named("lines", CellType::Output(sub_type.clone())),
        ])?;
    let files = parse(context.arguments, context.input.initialize()?)?;
    run(files, output)
}
