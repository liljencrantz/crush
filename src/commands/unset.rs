use crate::commands::CompileContext;
use crate::data::Argument;
use crate::data::Value;
use crate::errors::argument_error;
use crate::errors::CrushResult;

fn parse(arguments: Vec<Argument>) -> CrushResult<Vec<Box<str>>> {
    let mut vars = Vec::new();
    for arg in arguments.iter() {
        if let Value::Text(s) = &arg.value {
            if s.len() == 0 {
                return argument_error("Illegal variable name");
            } else {
                vars.push(s.clone());
            }
        } else {
            return argument_error("Illegal variable name");
        }
    }
    Ok(vars)
}

pub fn perform(context: CompileContext) -> CrushResult<()> {
    let vars = parse(context.arguments)?;
    context.output.initialize(vec![]);
    for s in vars {
        context.env.remove_str(s.as_ref());
    }
    Ok(())
}
