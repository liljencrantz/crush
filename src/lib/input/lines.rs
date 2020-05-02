use crate::lang::binary::BinaryReader;
use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::printer::Printer;
use crate::{
    lang::stream::OutputStream,
    lang::{table::ColumnType, table::Row, value::Value, value::ValueType},
};
use std::io::{BufRead, BufReader};

fn run(input: Box<dyn BinaryReader>, output: OutputStream, printer: &Printer) -> CrushResult<()> {
    let mut reader = BufReader::new(input);
    let mut line = String::new();
    loop {
        to_crush_error(reader.read_line(&mut line))?;
        if line.is_empty() {
            break;
        }
        let s = if line.ends_with('\n') {
            &line[0..line.len() - 1]
        } else {
            &line[..]
        };
        printer.handle_error(output.send(Row::new(vec![Value::string(s)])));
        line.clear();
    }
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let output = context
        .output
        .initialize(vec![ColumnType::new("line", ValueType::String)])?;
    run(context.reader()?, output, &context.printer)
}
