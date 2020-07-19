use std::io::{BufReader, BufRead};
use crate::lang::{
    execution_context::ExecutionContext,
    table::Row,
    table::ColumnType,
    value::ValueType,
    value::Value,
};
use crate::lang::errors::{CrushResult, to_crush_error};

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![ColumnType::new("line", ValueType::String)])?;
    let mut reader = BufReader::new(context.reader()?);
    let mut line = String::new();

    loop {
        to_crush_error(reader.read_line(&mut line))?;
        if line.is_empty() {
            break;
        }
        let s = if line.ends_with('\n') { &line[0..line.len() - 1] } else { &line[..] };
        context.printer.handle_error(output.send(Row::new(vec![Value::string(s)])));
        line.clear();
    }
    Ok(())
}
