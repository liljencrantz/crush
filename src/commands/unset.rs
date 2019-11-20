use crate::commands::CompileContext;
use crate::data::Argument;
use crate::data::Value;
use crate::errors::argument_error;
use crate::errors::JobResult;

fn parse(arguments: Vec<Argument>) -> JobResult<Vec<Box<str>>> {
    let mut vars = Vec::new();
    for arg in arguments.iter() {
        if let Value::Text(s) = &arg.value {
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

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let vars = parse(context.arguments)?;
    context.output.initialize(vec![]);
    for s in vars {
        context.env.remove_str(s.as_ref());
    }
    return Ok(());
}
