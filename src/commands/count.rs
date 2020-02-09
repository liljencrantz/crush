use crate::commands::CompileContext;
use crate::errors::{JobResult, error};
use crate::{
    data::{
        Row,
        ValueType,
        Value
    }
};
use crate::data::{ColumnType, RowsReader};
use crate::stream::{Readable};

fn count_rows(mut s: impl Readable) -> Value {
    let mut res: i128 = 0;
    loop {
        match s.read() {
            Ok(_) => res+= 1,
            Err(_) => break,
        }
    }
    return Value::Integer(res);
}

pub fn perform(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(vec![ColumnType::named("count", ValueType::Integer)])?;
    match context.input.recv()? {
        Value::Stream(s) => {
            let input = s.stream;
            output.send(Row::new(vec![count_rows(input)]))
        }
        Value::Rows(r) => {
            let input = RowsReader::new(r);
            output.send(Row::new(vec![count_rows(input)]))
        }
        _ => Err(error("Expected a stream")),
    }
}
