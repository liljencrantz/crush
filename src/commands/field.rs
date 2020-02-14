use crate::commands::CompileContext;
use crate::data::{Value, Argument};
use crate::data::Struct;
use crate::stream::ValueSender;
use crate::errors::{CrushResult, argument_error};
use crate::commands::command_util::find_field;
use crate::commands::parse_util::single_argument_field;

pub struct Config {
    columns: Vec<(usize, Option<Box<str>>)>,
}

fn perform_single(mut input: Struct, output: ValueSender, arguments: Vec<Argument>) -> CrushResult<()> {
    output.send(input.remove(find_field(&single_argument_field(arguments)?, &input.types())?))
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Struct(s) => {
            perform_single(s, context.output, context.arguments)
        }
        _ => argument_error("Expected a struct"),
    }
}
