use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;
use crate::lang::value::ValueType;
use crate::lang::command::OutputType::Known;

#[signature(
    stream.count,
    short = "Count the number of rows in the input.",
    output = Known(ValueType::Integer),
    example = "proc:list | count # Number of processes on the system")]
pub struct Count {}

pub fn count(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Table(r) => context.output.send(Value::Integer(r.len() as i128)),
        Value::List(r) => context.output.send(Value::Integer(r.len() as i128)),
        Value::Dict(r) => context.output.send(Value::Integer(r.len() as i128)),
        v => match v.stream()? {
            Some(mut input) => {
                let mut res: i128 = 0;
                while let Ok(_) = input.read() {
                    res += 1;
                }
                context.output.send(Value::Integer(res))
            }
            None => argument_error_legacy("Expected a stream"),
        },
    }
}
