use crate::commands::{CompileContext, JobJoinHandle};
use crate::errors::JobResult;
use crate::{
    stream::{OutputStream, InputStream},
    data::Row,
    data::Argument,
    data::ArgumentVecCompiler,
    errors::JobError
};
use crate::printer::Printer;
use crate::env::Env;
use crate::data::ColumnType;

pub fn run(mut arguments: Vec<Argument>, output: OutputStream) -> JobResult<()> {
    output.send(Row {
        cells: arguments.drain(..).map(|c| c.cell).collect()
    })
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output_type = context.arguments.iter().map(Argument::cell_type).collect();
    let output = context.output.initialize(output_type)?;
    run(context.arguments, output);
    Ok(())
}
