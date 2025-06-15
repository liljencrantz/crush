use crate::lang::errors::CrushResult;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "term",
        "Constants useful for manipulating the terminal, such as changing text color and text weight.",
        Box::new(move |fd| {
            fd.declare("normal", Value::from("\x1b[0m"))?;
            fd.declare("bold", Value::from("\x1b[1m"))?;
            fd.declare("underline", Value::from("\x1b[4m"))?;
            fd.declare("black", Value::from("\x1b[30m"))?;
            fd.declare("red", Value::from("\x1b[31m"))?;
            fd.declare("green", Value::from("\x1b[32m"))?;
            fd.declare("yellow", Value::from("\x1b[33m"))?;
            fd.declare("blue", Value::from("\x1b[34m"))?;
            fd.declare("magenta", Value::from("\x1b[35m"))?;
            fd.declare("cyan", Value::from("\x1b[36m"))?;
            fd.declare("white", Value::from("\x1b[37m"))?;
            Ok(())
        }),
    )?;
    Ok(())
}
