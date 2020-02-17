use crate::data::{Value, BinaryReader};
use crate::lib::ExecutionContext;
use crate::errors::CrushResult;
use crate::lib::parse_util::argument_files;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::BinaryReader(BinaryReader::paths(argument_files(context.arguments)?)?))
}
