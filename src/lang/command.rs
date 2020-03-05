use crate::errors::CrushResult;
use std::fmt::Formatter;
use crate::stream::{ValueReceiver, ValueSender, InputStream};
use crate::lang::{argument::Argument, argument::ArgumentDefinition};
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
    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, env: &Scope) -> bool;
}

#[derive(Clone)]
pub struct SimpleCommand {
    call: fn(context: ExecutionContext) -> CrushResult<()>,
    can_block: bool,
}

impl SimpleCommand {
    pub fn new(call: fn(context: ExecutionContext) -> CrushResult<()>, can_block: bool) -> SimpleCommand {
        return SimpleCommand { call, can_block };
    }
}

impl CrushCommand for SimpleCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn can_block(&self, _arg: &Vec<ArgumentDefinition>, _env: &Scope) -> bool {
        self.can_block
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

#[derive(Clone)]
pub struct ConditionCommand {
    call: fn(context: ExecutionContext) -> CrushResult<()>,
}

impl ConditionCommand {
    pub fn new(call: fn(context: ExecutionContext) -> CrushResult<()>) -> ConditionCommand {
        return ConditionCommand { call };
    }
}

impl CrushCommand for ConditionCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
        for arg in arguments {
            if arg.value.can_block(arguments, env) {
                return true;
            }
        }
        false
    }
}

impl std::cmp::PartialEq for ConditionCommand {
    fn eq(&self, _other: &ConditionCommand) -> bool {
        return false;
    }
}

impl std::cmp::Eq for ConditionCommand {}

impl std::fmt::Debug for ConditionCommand {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "Command")
    }
}
