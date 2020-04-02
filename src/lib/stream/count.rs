use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{CrushResult, error, mandate, argument_error};
use crate::lang::value::Value;
use crate::lang::stream::Readable;

fn count_rows(mut s: Box<dyn Readable>) -> Value {
    let mut res: i128 = 0;
    loop {
        match s.read() {
            Ok(_) => res += 1,
            Err(_) => break,
        }
    }
    return Value::Integer(res);
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()? {
        Value::Table(r) => context.output.send(Value::Integer(r.rows().len() as i128)),
        Value::List(r) => context.output.send(Value::Integer(r.len() as i128)),
        Value::Dict(r) => context.output.send(Value::Integer(r.len() as i128)),
        v =>
            match v.readable() {
                Some(readable) => context.output.send(count_rows(readable)),
                None =>  argument_error("Expected a stream")
            }
    }
}
