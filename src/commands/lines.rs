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
        ValueType,
        Output,
        Value,
    },
    stream::{OutputStream, InputStream, unlimited_streams},
};
use either::Either;
use crate::errors::JobResult;
use crate::commands::command_util::find_field;

lazy_static! {
    static ref sub_type: Vec<ColumnType> = {
        vec![ColumnType::named("line", ValueType::Text)]
    };
}

fn handle(file: Box<Path>, output: &OutputStream) -> JobResult<()> {
    let (output_stream, input_stream) = unlimited_streams(sub_type.clone());
    let out_row = Row {
        cells: vec![
            Value::File(file.clone()),
            Value::Output(Output {
                stream: input_stream,
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
            output_stream.send(Row { cells: vec![Value::Text(line[0..line.len() - 1].to_string().into_boxed_str())] });
            line.clear();
        }
    });
    return Ok(());
}

pub struct Config {
    files: Vec<Box<Path>>,
}

fn parse(arguments: Vec<Argument>) -> JobResult<Config> {
    let mut files: Vec<Box<Path>> = Vec::new();
    for arg in &arguments {
        arg.value.file_expand(&mut files)?;
    }
    Ok(Config { files: files })
}

pub fn run(config: Config, output: OutputStream) -> JobResult<()> {
    for file in config.files {
        handle(file, &output)?;
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(
        vec![
            ColumnType::named("file", ValueType::File),
            ColumnType::named("lines", ValueType::Output(sub_type.clone())),
        ])?;
    let files = parse(context.arguments)?;
    run(files, output)
}
