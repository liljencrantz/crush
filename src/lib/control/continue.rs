use crate::lang::ExecutionContext;
use crate::errors::CrushResult;
use crate::lang::Value;
use crate::scope::cwd;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    context.env.do_continue();
    Ok(())
}
