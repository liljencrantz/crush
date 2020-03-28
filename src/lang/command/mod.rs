mod closure;

use crate::lang::errors::{CrushResult, error, argument_error};
use std::fmt::Formatter;
use crate::lang::stream::{ValueReceiver, ValueSender};
use crate::lang::{argument::Argument, argument::ArgumentDefinition};
use crate::lang::scope::Scope;
use crate::lang::job::Job;
use crate::lang::value::{Value, ValueType, ValueDefinition};
use crate::lang::list::List;
use crate::lang::dict::Dict;
use crate::lang::r#struct::Struct;
use std::path::Path;
use crate::util::replace::Replace;
use regex::Regex;
use crate::util::glob::Glob;
use chrono::{Duration, Local, DateTime};
use closure::Closure;
use crate::lang::execution_context::ExecutionContext;

pub trait CrushCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()>;
    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, env: &Scope) -> bool;
    fn name(&self) -> &str;
    fn clone(&self) -> Box<dyn CrushCommand + Send + Sync>;
    fn help(&self) -> &str;
}


impl dyn CrushCommand {
    pub fn closure(signature: Option<Vec<Parameter>>, job_definitions: Vec<Job>, env: &Scope) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(Closure {
            signature,
            job_definitions,
            env: env.clone(),
        })
    }

    pub fn command(call: fn(context: ExecutionContext) -> CrushResult<()>, can_block: bool) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(SimpleCommand {
            call,
            can_block,
            help: "FDSAFASD",
        })
    }

    pub fn condition(call: fn(context: ExecutionContext) -> CrushResult<()>, help: &'static str) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(ConditionCommand { call, help })
    }
}

#[derive(Clone)]
struct SimpleCommand {
    call: fn(context: ExecutionContext) -> CrushResult<()>,
    can_block: bool,
    help: &'static str,
}


impl CrushCommand for SimpleCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn name(&self) -> &str { "command" }

    fn can_block(&self, _arg: &Vec<ArgumentDefinition>, _env: &Scope) -> bool {
        self.can_block
    }

    fn clone(&self) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(SimpleCommand { call: self.call, can_block: self.can_block, help: self.help })
    }

    fn help(&self) -> &str {
        self.help
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
struct ConditionCommand {
    call: fn(context: ExecutionContext) -> CrushResult<()>,
    help: &'static str,
}

impl CrushCommand for ConditionCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn name(&self) -> &str { "conditional command" }

    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
        for arg in arguments {
            if arg.value.can_block(arguments, env) {
                return true;
            }
        }
        false
    }

    fn clone(&self) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(ConditionCommand { call: self.call, help: self.help })
    }

    fn help(&self) -> &str {
        self.help
    }
}

impl std::cmp::PartialEq for ConditionCommand {
    fn eq(&self, _other: &ConditionCommand) -> bool {
        return false;
    }
}

impl std::cmp::Eq for ConditionCommand {}


#[derive(Clone)]
pub enum Parameter {
    Parameter(Box<str>, ValueDefinition, Option<ValueDefinition>),
    Named(Box<str>),
    Unnamed(Box<str>),
}
