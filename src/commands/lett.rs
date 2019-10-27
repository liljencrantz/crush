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
    context.output.initialize(vec![]);

    for arg in context.arguments.iter() {
        if arg.val_or_empty().is_empty() {
            return Err(
                argument_error("Missing variable name")
            );
        }
    }
    run(context.arguments, context.env)
}
