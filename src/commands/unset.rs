use crate::commands::CompileContext;
use crate::{
    data::Argument,
    errors::{JobError, argument_error},
    env::Env,
};
use crate::data::{Cell, ColumnType};
use crate::printer::Printer;
use crate::stream::{InputStream, OutputStream};
use crate::errors::JobResult;


fn parse(arguments: Vec<Argument>) -> JobResult<Vec<Box<str>>> {
    let mut vars = Vec::new();
    for arg in arguments.iter() {
        if let Cell::Text(s) = &arg.cell {
            if s.len() == 0 {
                return Err(argument_error("Illegal variable name"));
            } else {
                vars.push(s.clone());
            }
        } else {
            return Err(argument_error("Illegal variable name"));
        }
    }
    Ok(vars)
}

pub fn run(
    vars: Vec<Box<str>>,
    env: Env,
) -> JobResult<()> {
    for s in vars {
        env.remove(s.as_ref());
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let vars = parse(context.arguments)?;
    context.output.initialize(vec![]);
    run(vars, context.env)
}
