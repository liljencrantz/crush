use crate::lang::{Value, BinaryReader};
use crate::lang::ExecutionContext;
use crate::errors::CrushResult;
use crate::lib::parse_util::argument_files;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::BinaryStream(BinaryReader::paths(argument_files(context.arguments)?)?))
}
