use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::errors::error;
use crate::data::CellType;
use crate::data::Cell;
use crate::stream::OutputStream;
use crate::stream::Readable;

pub fn run(input1: &mut impl Readable, input2: &mut impl Readable, output: OutputStream) -> JobResult<()> {
    loop {
        match (input1.read(), input2.read()) {
            (Ok(mut row1), Ok(mut row2)) => {
                row1.cells.append(&mut row2.cells);
                output.send(row1)?;
            }
            _ => break,
        }
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize()?;
    let input_type = input.get_type();
    if input_type.len() != 2 {
        return Err(error("Expected exactly two arguments"));
    }
    match (&input_type[0].cell_type, &input_type[1].cell_type) {
        (CellType::Output(o1), CellType::Output(o2)) => {
            let mut output_type = Vec::new();
            output_type.append(&mut o1.clone());
            output_type.append(&mut o2.clone());
            let output = context.output.initialize(output_type)?;

            match input.recv() {
                Ok(mut row) => {
                    match (row.cells.remove(0), row.cells.remove(0)) {
                        (Cell::Output(mut r1), Cell::Output(mut r2)) => {
                            run(&mut r1.stream, &mut r2.stream, output)
                        }
                        _ => return Err(error("Expected two streams of data")),
                    }        
                }
                Err(_) => Ok(()),
            }
        }
        _ => return Err(error("Expected two streams of data")),

    }
}
