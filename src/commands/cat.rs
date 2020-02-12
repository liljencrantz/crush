use crate::data::{Argument, Value};
use crate::commands::CompileContext;
use crate::data::{BinaryReader};
use crate::errors::{argument_error};
use crate::errors::CrushResult;
use crate::stream::{ValueReceiver};

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> CrushResult<Box<dyn BinaryReader>> {
    match arguments.len() {
        1 => {
            let mut files = Vec::new();
            arguments[0].value.file_expand(&mut files);
            BinaryReader::from(&files.remove(0))
        }
        _ => Err(argument_error("Expected a file name"))
    }
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    let input = parse(context.arguments, context.input)?;
    context.output.send(Value::BinaryReader(input))
}
