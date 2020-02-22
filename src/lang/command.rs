use crate::errors::CrushResult;
use std::fmt::Formatter;
use crate::stream::{ValueReceiver, ValueSender, InputStream};
use crate::lang::Argument;
use crate::scope::Scope;
use crate::printer::Printer;

pub struct ExecutionContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub env: Scope,
    pub printer: Printer,
}

pub struct StreamExecutionContext {
    pub argument_stream: InputStream,
    pub output: ValueSender,
    pub env: Scope,
    pub printer: Printer,
}

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
