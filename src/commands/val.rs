use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::data::{Struct, Value};
use crate::data::Argument;

pub fn perform(mut context: CompileContext) -> JobResult<()> {
    context.output.send(context.arguments.remove(0).value)
}
