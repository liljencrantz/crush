use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use signature::signature;

#[signature(
    stream.count,
    short = "Count the number of rows in the input.",
    output = Known(ValueType::Integer),
    long = "The input type can be any type that can be streamed, such as a table, a list, etc.",
    long = "If the input type is not a materialized type, such as a `$table_input_stream`, the whole stream will be consumed by this operation.",
    long = "Materialized types, i.e. `$table`, `$list` and `$dict`, have a know size, and `count` will not need to iterate over them to find it.",
    example = "# Returns the number of processes on the system",
    example = "host:procs | count")]
pub struct Count {}

pub fn count(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Table(r) => context.output.send(Value::from(r.len())),
        Value::List(r) => context.output.send(Value::from(r.len())),
        Value::Dict(r) => context.output.send(Value::from(r.len())),
        v => {
            let mut input = v.stream(context.command_handle())?;
            let mut res: i128 = 0;
            while let Ok(_) = input.read() {
                res += 1;
            }
            context.output.send(Value::from(res))
        }
    }
}
