use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::{
    lang::{
        Row,
        ValueType,
        Value,
        ColumnType,
        RowsReader,
    }
};
use crate::stream::Readable;

fn count_rows(mut s: impl Readable) -> Value {
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
        Value::Stream(s) => context.output.send(count_rows(s.stream)),
        Value::Rows(r) => context.output.send(Value::Integer(r.rows().len() as i128)),
        Value::List(r) => context.output.send(Value::Integer(r.len() as i128)),
        Value::Dict(r) => context.output.send(Value::Integer(r.len() as i128)),
        _ => error("Expected a stream"),
    }
}
