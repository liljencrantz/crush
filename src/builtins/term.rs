use crate::lang::errors::CrushResult;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;

pub static RED: &'static str = "\x1b[31m";
pub static GREEN: &'static str = "\x1b[32m";
pub static BLUE: &'static str = "\x1b[34m";
pub static YELLOW: &'static str = "\x1b[33m";
pub static CYAN: &'static str = "\x1b[36m";
pub static MAGENTA: &'static str = "\x1b[35m";

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "term",
        "Constants useful for manipulating the terminal, such as changing text color and text weight.",
        Box::new(move |fd| {
            fd.declare("normal", Value::from("\x1b[0m"))?;
            fd.declare("bold", Value::from("\x1b[1m"))?;
            fd.declare("underline", Value::from("\x1b[4m"))?;
            fd.declare("black", Value::from("\x1b[30m"))?;
            fd.declare("red", Value::from(RED))?;
            fd.declare("green", Value::from(GREEN))?;
            fd.declare("yellow", Value::from(YELLOW))?;
            fd.declare("blue", Value::from(BLUE))?;
            fd.declare("magenta", Value::from(MAGENTA))?;
            fd.declare("cyan", Value::from(CYAN))?;
            fd.declare("white", Value::from("\x1b[37m"))?;
            Ok(())
        }),
    )?;
    Ok(())
}
