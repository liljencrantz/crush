use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, argument_error_legacy};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use signature::signature;

#[signature(
    stream.count,
    short = "Count the number of rows in the input.",
    output = Known(ValueType::Integer),
    example = "host:procs | count # Number of processes on the system")]
pub struct Count {}

pub fn count(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Table(r) => context.output.send(Value::from(r.len())),
        Value::List(r) => context.output.send(Value::from(r.len())),
        Value::Dict(r) => context.output.send(Value::from(r.len())),
        v => match v.stream()? {
            Some(mut input) => {
                let mut res: i128 = 0;
                while let Ok(_) = input.read() {
                    res += 1;
                }
                context.output.send(Value::from(res))
            }
            None => argument_error_legacy("`count`: Expected a stream"),
        },
    }
}
