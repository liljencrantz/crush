use crate::data::{Value, BinaryReader};
use crate::commands::CompileContext;
use crate::errors::CrushResult;
use crate::commands::parse_util::argument_files;

pub fn perform(context: CompileContext) -> CrushResult<()> {
    context.output.send(Value::BinaryReader(BinaryReader::paths(argument_files(context.arguments)?)?))
}
