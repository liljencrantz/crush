use crate::{
    data::{
        Argument,
        Value,
        ValueType,
    },
    errors::JobError,
    stream::{OutputStream},
};
use crate::commands::command_util::{find_field_from_str, find_field};
use crate::commands::CompileContext;
use crate::data::{ColumnType, BinaryReader};
use crate::errors::{argument_error, error};
use crate::errors::JobResult;
use crate::replace::Replace;
use crate::stream::{RowsReader, Readable, ValueReceiver};

pub struct Config {
    column: usize,
}

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> JobResult<BinaryReader> {
    match arguments.len() {
        1 => {
            let mut files = Vec::new();
            arguments[0].value.file_expand(&mut files);
            Ok(BinaryReader::from(&files.remove(0))?)
        }
        _ => Err(argument_error("Expected a file name"))
    }
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    let input = parse(context.arguments, context.input)?;
    context.output.send(Value::BinaryReader(input))
}
