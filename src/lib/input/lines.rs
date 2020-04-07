use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use std::io::{BufReader, BufRead};
use crate::{
    lang::errors::argument_error,
    lang::{
        table::Row,
        table::ColumnType,
        value::ValueType,
        value::Value,
    },
    lang::stream::OutputStream,
};
use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::binary::BinaryReader;

fn run(input: Box<dyn BinaryReader>, output: OutputStream) -> CrushResult<()> {
    let mut reader = BufReader::new(input);
    let mut line = String::new();
    loop {
        to_crush_error(reader.read_line(&mut line))?;
        if line.is_empty() {
            break;
        }
        let _ = output.send(Row::new(vec![Value::String(line[0..line.len() - 1].to_string().into_boxed_str())]));
        line.clear();
    }
    Ok(())
}


pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![ColumnType::new("line", ValueType::String)])?;
    let file = match context.arguments.len() {
        0 => {
            let v = context.input.recv()?;
            match v {
                Value::BinaryStream(b) => Ok(b),
                Value::Binary(b) => Ok(BinaryReader::vec(&b)),
                _ => argument_error("Expected either a file to read or binary pipe input"),
            }
        }
        _ => BinaryReader::paths(context.arguments.files(&context.printer)?),
    }?;
    run(file, output)
}
