use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::data::{Struct, Value};
use crate::data::Argument;

pub fn perform(mut context: CompileContext) -> JobResult<()> {
    context.output.send(
        Value::Struct(Struct {
            types: context.arguments.iter().map(Argument::cell_type).collect(),
            cells: context.arguments.drain(..).map(|c| c.value).collect(),
        }))
}
