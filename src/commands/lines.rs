use crate::commands::CompileContext;
use std::io::{BufReader, BufRead};
use crate::{
    errors::argument_error,
    data::{
        Argument,
        Row,
        ColumnType,
        ValueType,
        Value,
    },
    stream::OutputStream,
};
use crate::stream::ValueReceiver;
use crate::errors::CrushResult;
use crate::data::BinaryReader;

fn run(input: Box<dyn BinaryReader>, output: OutputStream) -> CrushResult<()> {
    let mut reader = BufReader::new(input.reader());
    let mut line = String::new();
    loop {
        reader.read_line(&mut line);
        if line.is_empty() {
            break;
        }
        output.send(Row::new(vec![Value::Text(line[0..line.len() - 1].to_string().into_boxed_str())]));
        line.clear();
    }
    return Ok(());
}

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> CrushResult<Box<dyn BinaryReader>> {
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
            BinaryReader::from(&files.remove(0))
        }
        _ => Err(argument_error("Expected a file name"))
    }
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![ColumnType::named("line", ValueType::Text)])?;
    let file = parse(context.arguments, context.input)?;
    run(file, output)
}
