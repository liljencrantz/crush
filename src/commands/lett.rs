use crate::commands::{CompileContext, JobJoinHandle};
use crate::errors::JobResult;
use crate::{
    data::{Argument},
    errors::{JobError, argument_error},
    env::Env
};
use crate::stream::{OutputStream, InputStream};
use crate::printer::Printer;
use crate::data::{ColumnType, ArgumentVecCompiler};

pub fn run(arguments: Vec<Argument>, env: Env) -> JobResult<()> {
    for arg in arguments {
        env.declare(arg.name.unwrap().as_ref(), arg.cell)?;
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let mut deps: Vec<JobJoinHandle> = Vec::new();
    let arguments = context.argument_definitions.compile(&mut deps, &context)?;
    context.output.initialize(vec![]);

    for arg in arguments.iter() {
        if arg.val_or_empty().is_empty() {
            return Err(
                argument_error("Missing variable name")
            );
        }
    }
    run(arguments, context.env)
}
