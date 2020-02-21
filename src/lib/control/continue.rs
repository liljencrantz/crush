use crate::lib::ExecutionContext;
use crate::errors::CrushResult;
use crate::data::Value;
use crate::namespace::cwd;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.env.do_continue();
    Ok(())
}
