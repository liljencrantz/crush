use crate::lang::command::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::{
    lang::{
        table::Row,
        value::ValueType,
        value::Value,
        table::ColumnType,
        table::TableReader,
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
        Value::TableStream(s) => context.output.send(count_rows(s.stream)),
        Value::Table(r) => context.output.send(Value::Integer(r.rows().len() as i128)),
        Value::List(r) => context.output.send(Value::Integer(r.len() as i128)),
        Value::Dict(r) => context.output.send(Value::Integer(r.len() as i128)),
        _ => error("Expected a stream"),
    }
}
