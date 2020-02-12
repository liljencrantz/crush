use crate::commands::CompileContext;
use crate::data::{Value, Argument};
use crate::data::Struct;
use crate::stream::ValueSender;
use crate::errors::{CrushResult, argument_error};
use crate::commands::command_util::find_field;

pub struct Config {
    columns: Vec<(usize, Option<Box<str>>)>,
}

fn perform_single(mut input: Struct, output: ValueSender, arguments: Vec<Argument>) -> CrushResult<()> {
    if arguments.len() == 1 && arguments[0].name.is_none() {
        match &arguments[0].value {
            Value::Field(s) => output.send(input.remove(find_field(s, &input.types())?)),
            _ => Err(argument_error("Expected Field")),
        }
    } else {
        Err(argument_error("Specify exactly one field to access"))
    }
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Struct(s) => {
            perform_single(s, context.output, context.arguments)
        }
        _ => Err(argument_error("Expected a struct")),
    }
}
