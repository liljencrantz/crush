use crate::lang::command::ExecutionContext;
use crate::lang::errors::CrushResult;
use crate::lang::errors::error;
use crate::lang::value::Value;
use crate::lang::stream::{ValueSender};
use crate::lang::stream::Readable;

pub fn run(input1: &mut impl Readable, input2: &mut impl Readable, sender: ValueSender) -> CrushResult<()> {
    let mut output_type = Vec::new();
    output_type.append(&mut input1.types().clone());
    output_type.append(&mut input2.types().clone());
    let output = sender.initialize(output_type)?;
    loop {
        match (input1.read(), input2.read()) {
            (Ok(mut row1), Ok(mut row2)) => {
                row1.append(&mut row2.into_vec());
                output.send(row1)?;
            }
            _ => break,
        }
    }
    return Ok(());
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::TableStream(mut o1), Value::TableStream(mut o2)) =>
            run(&mut o1, &mut o2, context.output),
        (Value::Table(mut o1), Value::Table(mut o2)) =>
            run(&mut o1.reader(), &mut o2.reader(), context.output),
        (Value::TableStream(mut o1), Value::Table(mut o2)) =>
            run(&mut o1, &mut o2.reader(), context.output),
        (Value::Table(mut o1), Value::TableStream(mut o2)) =>
            run(&mut o1.reader(), &mut o2, context.output),
        _ => return error("Expected two datasets"),
    }
}
