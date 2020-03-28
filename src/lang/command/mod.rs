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

pub trait ArgumentVector {
    fn check_len(&self, len: usize) -> CrushResult<()>;
    fn string(&mut self, idx: usize) -> CrushResult<Box<str>>;
    fn integer(&mut self, idx: usize) -> CrushResult<i128>;
    fn field(&mut self, idx: usize) -> CrushResult<Vec<Box<str>>>;
    fn command(&mut self, idx: usize) -> CrushResult<Box<dyn CrushCommand + Send + Sync>>;
    fn r#type(&mut self, idx: usize) -> CrushResult<ValueType>;
    fn value(&mut self, idx: usize) -> CrushResult<Value>;
    fn glob(&mut self, idx: usize) -> CrushResult<Glob>;
    fn files(&mut self) -> CrushResult<Vec<Box<Path>>>;
    fn optional_integer(&mut self) -> CrushResult<Option<i128>>;
}

impl ArgumentVector for Vec<Argument> {
    fn check_len(&self, len: usize) -> CrushResult<()> {
        if self.len() == len {
            Ok(())
        } else {
            argument_error(format!("Expected {} arguments, got {}", len, self.len()).as_str())
        }
    }

    fn string(&mut self, idx: usize) -> CrushResult<Box<str>> {
        if idx < self.len() {
            match self.replace(idx, Argument::unnamed(Value::Bool(false))).value {
                Value::String(s) => Ok(s),
                _ => error("Invalid value"),
            }
        } else {
            error("Index out of bounds")
        }
    }

    fn integer(&mut self, idx: usize) -> CrushResult<i128> {
        if idx < self.len() {
            match self.replace(idx, Argument::unnamed(Value::Bool(false))).value {
                Value::Integer(s) => Ok(s),
                _ => error("Invalid value"),
            }
        } else {
            error("Index out of bounds")
        }
    }

    fn field(&mut self, idx: usize) -> CrushResult<Vec<Box<str>>> {
        if idx < self.len() {
            match self.replace(idx, Argument::unnamed(Value::Bool(false))).value {
                Value::Field(s) => Ok(s),
                _ => error("Invalid value"),
            }
        } else {
            error("Index out of bounds")
        }
    }

    fn command(&mut self, idx: usize) -> CrushResult<Box<dyn CrushCommand + Send + Sync>> {
        if idx < self.len() {
            match self.replace(idx, Argument::unnamed(Value::Bool(false))).value {
                Value::Command(s) => Ok(s),
                _ => error("Invalid value"),
            }
        } else {
            error("Index out of bounds")
        }
    }

    fn r#type(&mut self, idx: usize) -> CrushResult<ValueType> {
        if idx < self.len() {
            match self.replace(idx, Argument::unnamed(Value::Bool(false))).value {
                Value::Type(s) => Ok(s),
                _ => error("Invalid value"),
            }
        } else {
            error("Index out of bounds")
        }
    }

    fn value(&mut self, idx: usize) -> CrushResult<Value> {
        if idx < self.len() {
            Ok(self.replace(idx, Argument::unnamed(Value::Bool(false))).value)
        } else {
            error("Index out of bounds")
        }
    }

    fn glob(&mut self, idx: usize) -> CrushResult<Glob> {
        if idx < self.len() {
            match self.replace(idx, Argument::unnamed(Value::Bool(false))).value {
                Value::Glob(s) => Ok(s),
                _ => error("Invalid value"),
            }
        } else {
            error("Index out of bounds")
        }
    }

    fn files(&mut self) -> CrushResult<Vec<Box<Path>>> {
        let mut files = Vec::new();
        for a in self.drain(..) {
            a.value.file_expand(&mut files)?;
        }
        Ok(files)
    }

    fn optional_integer(&mut self) -> CrushResult<Option<i128>> {
        match self.len() {
            0 => Ok(None),
            1 => {
                let a = self.remove(0);
                match (a.argument_type, a.value) {
                    (None, Value::Integer(i)) => Ok(Some(i)),
                    _ => argument_error("Expected a text value"),
                }
            }
            _ => argument_error("Expected a single value"),
        }
    }
}

pub struct ExecutionContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub env: Scope,
    pub this: Option<Value>,
}

pub trait This {
    fn list(self) -> CrushResult<List>;
    fn dict(self) -> CrushResult<Dict>;
    fn text(self) -> CrushResult<Box<str>>;
    fn r#struct(self) -> CrushResult<Struct>;
    fn file(self) -> CrushResult<Box<Path>>;
    fn re(self) -> CrushResult<(Box<str>, Regex)>;
    fn glob(self) -> CrushResult<Glob>;
    fn integer(self) -> CrushResult<i128>;
    fn float(self) -> CrushResult<f64>;
    fn r#type(self) -> CrushResult<ValueType>;
    fn duration(self) -> CrushResult<Duration>;
    fn time(self) -> CrushResult<DateTime<Local>>;
}


impl This for Option<Value> {
    fn list(mut self) -> CrushResult<List> {
        match self.take() {
            Some(Value::List(l)) => Ok(l),
            _ => argument_error("Expected a list"),
        }
    }

    fn dict(mut self) -> CrushResult<Dict> {
        match self.take() {
            Some(Value::Dict(l)) => Ok(l),
            _ => argument_error("Expected a dict"),
        }
    }

    fn text(mut self) -> CrushResult<Box<str>> {
        match self.take() {
            Some(Value::String(s)) => Ok(s),
            _ => argument_error("Expected a string"),
        }
    }

    fn r#struct(mut self) -> CrushResult<Struct> {
        match self.take() {
            Some(Value::Struct(s)) => Ok(s),
            _ => argument_error("Expected a struct"),
        }
    }

    fn file(mut self) -> CrushResult<Box<Path>> {
        match self.take() {
            Some(Value::File(s)) => Ok(s),
            _ => argument_error("Expected a file"),
        }
    }

    fn re(mut self) -> CrushResult<(Box<str>, Regex)> {
        match self.take() {
            Some(Value::Regex(s, b)) => Ok((s, b)),
            _ => argument_error("Expected a regular expression"),
        }
    }

    fn glob(mut self) -> CrushResult<Glob> {
        match self.take() {
            Some(Value::Glob(s)) => Ok(s),
            _ => argument_error("Expected a glob"),
        }
    }

    fn integer(mut self) -> CrushResult<i128> {
        match self.take() {
            Some(Value::Integer(s)) => Ok(s),
            _ => argument_error("Expected an integer"),
        }
    }

    fn float(mut self) -> CrushResult<f64> {
        match self.take() {
            Some(Value::Float(s)) => Ok(s),
            _ => argument_error("Expected a float"),
        }
    }

    fn r#type(mut self) -> CrushResult<ValueType> {
        match self.take() {
            Some(Value::Type(s)) => Ok(s),
            _ => argument_error("Expected a type"),
        }
    }

    fn duration(mut self) -> CrushResult<Duration> {
        match self.take() {
            Some(Value::Duration(s)) => Ok(s),
            _ => argument_error("Expected a duration"),
        }
    }

    fn time(mut self) -> CrushResult<DateTime<Local>> {
        match self.take() {
            Some(Value::Time(s)) => Ok(s),
            _ => argument_error("Expected a time"),
        }
    }
}
/*
pub struct StreamExecutionContext {
    pub argument_stream: InputStream,
    pub output: ValueSender,
    pub env: Scope,
}
*/
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
