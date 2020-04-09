mod closure;

use crate::lang::errors::{CrushResult, error, CrushError};
use std::fmt::Formatter;
use crate::lang::{argument::ArgumentDefinition};
use crate::lang::scope::Scope;
use crate::lang::job::Job;
use crate::lang::value::{ValueDefinition, Value};
use closure::Closure;
use crate::lang::execution_context::{ExecutionContext, CompileContext};
use crate::lang::help::Help;
use std::collections::HashMap;
use crate::lang::serialization::{SerializationState, DeserializationState};
use crate::lang::serialization::model::{Element, element, Strings};
use crate::lang::argument::ArgumentType;

pub trait CrushCommand : Help {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()>;
    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, context: &mut CompileContext) -> bool;
    fn name(&self) -> &str;
    fn clone(&self) -> Box<dyn CrushCommand +  Send + Sync>;
    fn help(&self) -> &dyn Help;
    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize>;
    fn bind(&self, this: Value) -> Box<dyn CrushCommand +  Send + Sync>;
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
                    CrushCommand::command(
                        call, can_block, path.iter().map(|e| e.to_string().into_boxed_str()).collect(),
                        signature, short_help, long_help),
        );
    }
}

struct SimpleCommand {
    call: fn(context: ExecutionContext) -> CrushResult<()>,
    can_block: bool,
    full_name: Vec<Box<str>>,
    signature: &'static str,
    short_help: &'static str,
    long_help: Option<&'static str>,
}

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

    pub fn command(
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
        full_name: Vec<Box<str>>,
        signature: &'static str,
        short_help: &'static str,
        long_help: Option<&'static str>,
    ) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(ConditionCommand { call, full_name, signature, short_help, long_help })
    }

    pub fn deserialize(
        id: usize,
        elements: &Vec<Element>,
        state: &mut DeserializationState,
    ) -> CrushResult<Box<dyn CrushCommand + Send + Sync>> {
        match elements[id].element.as_ref().unwrap() {
            element::Element::Command(strings) => {
                let val = state.env.global_value(
                    strings.elements
                        .iter()
                        .map(|e| e.clone().into_boxed_str())
                        .collect())?;
                match val {
                    Value::Command(c) => Ok(c),
                    _ => error("Expected a command"),
                }
            },
            _ => error("Expected a command"),
        }
    }

}

impl CrushCommand for SimpleCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let c = self.call;
        c(context)
    }

    fn name(&self) -> &str { "command" }

    fn can_block(&self, _arg: &Vec<ArgumentDefinition>, _context: &mut CompileContext) -> bool {
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

    fn serialize(&self, elements: &mut Vec<Element>, _state: &mut SerializationState) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Command(
                Strings{ elements: self.full_name.iter().map(|e| e.to_string()).collect()}
            )),
        });
        Ok(idx)
    }

    fn bind(&self, this: Value) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(BoundCommand {
            command: self.clone(),
            this,
        })
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
        false
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

    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, context: &mut CompileContext) -> bool {
        for arg in arguments {
            if arg.value.can_block(arguments, context) {
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

    fn serialize(&self, elements: &mut Vec<Element>, _state: &mut SerializationState) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Command(
                Strings{ elements: self.full_name.iter().map(|e| e.to_string()).collect()}
            )),
        });
        Ok(idx)
    }

    fn bind(&self, this: Value) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(BoundCommand {
            command: self.clone(),
            this,
        })
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
        false
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

pub struct BoundCommand {
    command: Box<dyn CrushCommand +  Send + Sync>,
    this: Value,
}

impl CrushCommand for BoundCommand {
    fn invoke(&self, mut context: ExecutionContext) -> CrushResult<()> {
        context.this = Some(self.this.clone());
        self.command.invoke(context)
    }

    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, context: &mut CompileContext) -> bool {
        self.command.can_block(arguments, context)
    }

    fn name(&self) -> &str {
        self.command.name()
    }

    fn clone(&self) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(
            BoundCommand {
                command: self.command.clone(),
                this: self.this.clone(),
            }
        )
    }

    fn help(&self) -> &Help {
        self.command.help()
    }

    fn serialize(&self, elements: &mut Vec<Element>, state: &mut SerializationState) -> CrushResult<usize> {
        unimplemented!()
    }

    fn bind(&self, this: Value) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(
            BoundCommand {
                command: self.command.clone(),
                this: this.clone(),
            }
        )
    }
}

impl Help for BoundCommand {
    fn signature(&self) -> String {
        self.command.signature()
    }

    fn short_help(&self) -> String {
        self.command.short_help()
    }

    fn long_help(&self) -> Option<String> {
        self.command.long_help()
    }
}
