mod closure;

use crate::lang::errors::{CrushResult};
use std::fmt::Formatter;
use crate::lang::{argument::ArgumentDefinition};
use crate::lang::scope::Scope;
use crate::lang::job::Job;
use crate::lang::value::{ValueDefinition};
use closure::Closure;
use crate::lang::execution_context::ExecutionContext;
use crate::lang::help::Help;
use std::collections::HashMap;

pub trait CrushCommand : Help {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()>;
    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, env: &Scope) -> bool;
    fn name(&self) -> &str;
    fn clone(&self) -> Box<dyn CrushCommand +  Send + Sync>;
    fn help(&self) -> &dyn Help;
}

pub trait TypeMap {
    fn declare(
        &mut self,
        path: Vec<&str>,
        call: fn(context: ExecutionContext) -> CrushResult<()>,
        can_block: bool,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    );
}

impl TypeMap for HashMap<Box<str>, Box<dyn CrushCommand + Sync + Send>> {
    fn declare(
        &mut self,
        path: Vec<&str>,
        call: fn(ExecutionContext) -> CrushResult<()>,
        can_block: bool,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) {
        self.insert(Box::from(path[path.len() - 1]),
                    CrushCommand::command2(
                        call, can_block, path.iter().map(|e| e.to_string().into_boxed_str()).collect(),
                        signature, short_help, long_help),
        );
    }
}

#[derive(Clone)]
struct SimpleCommand {
    call: fn(context: ExecutionContext) -> CrushResult<()>,
    can_block: bool,
    full_name: Vec<Box<str>>,
    signature: &'static str,
    short_help: &'static str,
    long_help: Option<&'static str>,
}

#[derive(Clone)]
struct ConditionCommand {
    call: fn(context: ExecutionContext) -> CrushResult<()>,
    full_name: Vec<Box<str>>,
    signature: &'static str,
    short_help: &'static str,
    long_help: Option<&'static str>,
}

impl dyn CrushCommand {
    pub fn closure(
        name: Option<Box<str>>,
        signature: Option<Vec<Parameter>>,
        job_definitions: Vec<Job>,
        env: &Scope,
    ) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(Closure::new(
            name,
            signature,
            job_definitions,
            env.clone(),
        ))
    }
    pub fn command_undocumented(
        call: fn(context: ExecutionContext) -> CrushResult<()>,
        can_block: bool,
//        full_name: Vec<Box<str>>,
    ) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(SimpleCommand { call, can_block, signature: "", short_help: "", long_help: None, /*full_name*/ full_name: vec![] })
    }

    pub fn command(
        call: fn(context: ExecutionContext) -> CrushResult<()>,
        can_block: bool,
//        full_name: Vec<Box<str>>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(SimpleCommand { call, can_block, full_name: vec![], signature, short_help, long_help })
    }

    pub fn command2(
        call: fn(context: ExecutionContext) -> CrushResult<()>,
        can_block: bool,
        full_name: Vec<Box<str>>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(SimpleCommand { call, can_block, full_name, signature, short_help, long_help })
    }

    pub fn condition(
        call: fn(context: ExecutionContext) -> CrushResult<()>,
//        full_name: Vec<Box<str>>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(ConditionCommand { call, full_name: vec![], signature, short_help, long_help })
    }
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

    fn clone(&self) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(SimpleCommand {
            call: self.call,
            can_block: self.can_block,
            full_name: self.full_name.clone(),
            signature: self.signature,
            short_help: self.short_help,
            long_help: self.long_help,
        })
    }

    fn help(&self) -> &dyn Help {
        self
    }
}

impl Help for SimpleCommand {
    fn signature(&self) -> String {
        self.signature.to_string()
    }

    fn short_help(&self) -> String {
        self.short_help.to_string()
    }

    fn long_help(&self) -> Option<String> {
        self.long_help.map(|s| s.to_string())
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

    fn clone(&self) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(ConditionCommand {
            call: self.call,
            full_name: self.full_name.clone(),
            signature: self.signature,
            short_help: self.short_help,
            long_help: self.long_help,
        })
    }

    fn help(&self) -> &dyn Help {
        self
    }
}

impl Help for ConditionCommand {
    fn signature(&self) -> String {
        self.signature.to_string()
    }

    fn short_help(&self) -> String {
        self.short_help.to_string()
    }

    fn long_help(&self) -> Option<String> {
        self.long_help.map(|s| s.to_string())
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

impl ToString for Parameter {
    fn to_string(&self) -> String {
        match self {
            Parameter::Parameter(
                name,
                value_type,
                default) => format!(
                "{}:{}{}",
                name,
                value_type.to_string(),
                default.as_ref().map(|d| format!("={}", d.to_string())).unwrap_or("".to_string())),
            Parameter::Named(n) => format!("@@{}", n),
            Parameter::Unnamed(n) => format!("@{}", n),
        }
    }
}
