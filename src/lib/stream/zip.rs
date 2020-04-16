use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::stream::{ValueSender, Readable};

fn run(input1: &mut dyn Readable, input2: &mut dyn Readable, sender: ValueSender) -> CrushResult<()> {
    let mut output_type = Vec::new();
    output_type.append(&mut input1.types().to_vec());
    output_type.append(&mut input2.types().to_vec());
    let output = sender.initialize(output_type)?;
    while let (Ok(mut row1), Ok(row2)) = (input1.read(), input2.read()) {
        row1.append(&mut row2.into_vec());
        output.send(row1)?;
    }
    Ok(())
}

pub fn zip(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(2)?;
    match (context.arguments.value(0)?.readable(), context.arguments.value(1)?.readable()) {
        (Some(mut o1), Some(mut o2)) =>
            run(o1.as_mut(), o2.as_mut(), context.output),
        _ => argument_error("Expected two datasets"),
    }
}
