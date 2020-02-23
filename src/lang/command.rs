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

pub trait CrushCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()>;
}

#[derive(Clone)]
pub struct SimpleCommand {
    pub call: fn(context: ExecutionContext) -> CrushResult<()>,
}

impl SimpleCommand {
    pub fn new(call: fn(context: ExecutionContext) -> CrushResult<()>) -> SimpleCommand {
        return SimpleCommand { call };
    }
}

impl CrushCommand for SimpleCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }
}

impl std::cmp::PartialEq for SimpleCommand {
    fn eq(&self, _other: &SimpleCommand) -> bool {
        return false;
    }
}

impl std::cmp::Eq for SimpleCommand {}

impl std::fmt::Debug for SimpleCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command")
    }
}
