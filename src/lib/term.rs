use crate::lang::data::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::value::Value;

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "term",
        Box::new(move |fd| {
            fd.declare("normal", Value::string("\x1b[0m"))?;
            fd.declare("bold", Value::string("\x1b[1m"))?;
            fd.declare("underline", Value::string("\x1b[4m"))?;
            fd.declare("black", Value::string("\x1b[30m"))?;
            fd.declare("red", Value::string("\x1b[31m"))?;
            fd.declare("green", Value::string("\x1b[32m"))?;
            fd.declare("yellow", Value::string("\x1b[33m"))?;
            fd.declare("blue", Value::string("\x1b[34m"))?;
            fd.declare("magenta", Value::string("\x1b[35m"))?;
            fd.declare("cyan", Value::string("\x1b[36m"))?;
            fd.declare("white", Value::string("\x1b[37m"))?;
            Ok(())
        }),
    )?;
    Ok(())
}
