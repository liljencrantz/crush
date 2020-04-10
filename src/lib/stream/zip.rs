use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::CrushResult;
use crate::lang::errors::error;
use crate::lang::stream::ValueSender;
use crate::lang::stream::Readable;

pub fn run(input1: &mut dyn Readable, input2: &mut dyn Readable, sender: ValueSender) -> CrushResult<()> {
    let mut output_type = Vec::new();
    output_type.append(&mut input1.types().clone());
    output_type.append(&mut input2.types().clone());
    let output = sender.initialize(output_type)?;
    while let (Ok(mut row1), Ok(row2)) = (input1.read(), input2.read()) {
        row1.append(&mut row2.into_vec());
        output.send(row1)?;
    }
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return error("Expected exactly two arguments");
    }
    match (context.arguments.remove(0).value.readable(), context.arguments.remove(0).value.readable()) {
        (Some(mut o1), Some(mut o2)) =>
            run(o1.as_mut(), o2.as_mut(), context.output),
        _ => return error("Expected two datasets"),
    }
}
