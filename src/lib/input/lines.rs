use crate::lang::command::ExecutionContext;
use std::io::{BufReader, BufRead};
use crate::{
    lang::errors::argument_error,
    lang::{
        argument::Argument,
        table::Row,
        table::ColumnType,
        value::ValueType,
        value::Value,
    },
    lang::stream::OutputStream,
};
use crate::lang::stream::ValueReceiver;
use crate::lang::errors::CrushResult;
use crate::lang::binary::BinaryReader;
use crate::lib::parse_util::argument_files;

fn run(input: Box<dyn BinaryReader>, output: OutputStream) -> CrushResult<()> {
    let mut reader = BufReader::new(input);
    let mut line = String::new();
    loop {
        reader.read_line(&mut line);
        if line.is_empty() {
            break;
        }
        output.send(Row::new(vec![Value::String(line[0..line.len() - 1].to_string().into_boxed_str())]));
        line.clear();
    }
    Ok(())
}

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> CrushResult<Box<dyn BinaryReader + Send + Sync>> {
    match arguments.len() {
        0 => {
            let v = input.recv()?;
            match v {
                Value::BinaryStream(b) => Ok(b),
                Value::Binary(b) => Ok(BinaryReader::vec(&b)),
                _ => argument_error("Expected either a file to read or binary pipe input"),
            }
        }
        _ => BinaryReader::paths(argument_files(arguments)?),
    }
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![ColumnType::new("line", ValueType::String)])?;
    let file = parse(context.arguments, context.input)?;
    run(file, output)
}
