use crate::errors::CrushResult;
use crate::lib::ExecutionContext;
use std::fmt::Formatter;

#[derive(Clone)]
pub struct Command {
    pub call: fn(context: ExecutionContext) -> CrushResult<()>,
}

impl Command {
    pub fn new(call: fn(context: ExecutionContext) -> CrushResult<()>) -> Command {
        return Command { call };
    }
}

impl std::cmp::PartialEq for Command {
    fn eq(&self, _other: &Command) -> bool {
        return false;
    }
}

impl std::cmp::Eq for Command {}

impl std::fmt::Debug for Command {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command")
    }
}
