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
use crate::errors::JobResult;

lazy_static! {
    static ref sub_type: Vec<ColumnType> = {
        vec![ColumnType::named("line", ValueType::Text)]
    };
}

fn run(file: Box<Path>, output: OutputStream) -> JobResult<()> {
    let fff = File::open(file).unwrap();
    let mut reader = BufReader::new(&fff);
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

fn parse(arguments: Vec<Argument>) -> JobResult<Box<Path>> {
    let mut files: Vec<Box<Path>> = Vec::new();
    for arg in &arguments {
        arg.value.file_expand(&mut files)?;
    }
    if files.len() != 1 {
        return Err(argument_error("Expected exactly one file"));
    }
    Ok(files.remove(0))
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(sub_type.clone())?;
    let file = parse(context.arguments)?;
    run(file, output)
}
