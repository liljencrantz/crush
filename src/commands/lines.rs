use crate::commands::CompileContext;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std::path::Path;
use lazy_static::lazy_static;
use crate::{
    errors::argument_error,
    data::{
        Argument,
        Row,
        ColumnType,
        ValueType,
        Value,
    },
    stream::{OutputStream},
};
use crate::errors::{JobResult, to_job_error};
use crate::data::BinaryReader;
use crate::stream::ValueReceiver;

lazy_static! {
    static ref sub_type: Vec<ColumnType> = {
        vec![ColumnType::named("line", ValueType::Text)]
    };
}

fn run(input: BinaryReader, output: OutputStream) -> JobResult<()> {
    let mut reader = BufReader::new(input.reader);
    let mut line = String::new();
    loop {
        reader.read_line(&mut line);
        if line.is_empty() {
            break;
        }
        output.send(Row { cells: vec![Value::Text(line[0..line.len() - 1].to_string().into_boxed_str())] });
        line.clear();
    }
    return Ok(());
}

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> JobResult<BinaryReader> {
    match arguments.len() {
        0 => {
            let v = input.recv()?;
            match v {
                Value::BinaryReader(b) => {
                    Ok(b)
                }
                _ => Err(argument_error("Expected either a file to read or binary pipe input"))
            }
        }
        1 => {
            let mut files = Vec::new();
            arguments[0].value.file_expand(&mut files);
            Ok(BinaryReader::from(&files.remove(0))?)
        }
        _ => Err(argument_error("Expected a file name"))
    }
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(sub_type.clone())?;
    let file = parse(context.arguments, context.input)?;
    run(file, output)
}
