use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::data::Row;
use crate::data::Argument;

pub fn compile_and_run(mut context: CompileContext) -> JobResult<()> {
    let output_type = context.arguments.iter().map(Argument::cell_type).collect();
    let output = context.output.initialize(output_type)?;
    output.send(Row {
        cells: context.arguments.drain(..).map(|c| c.value).collect()
    })
}
