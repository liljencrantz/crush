use crate::lang::errors::{CrushResult, error, argument_error, CrushError};
use std::fmt::Formatter;
use crate::lang::stream::{ValueReceiver, ValueSender, InputStream, empty_channel};
use crate::lang::{argument::Argument, argument::ArgumentDefinition};
use crate::lang::scope::Scope;
use crate::lang::job::Job;
use crate::lang::stream_printer::spawn_print_thread;
use crate::lang::value::{Value, ValueType, ValueDefinition};
use crate::lang::list::List;
use crate::lang::dict::Dict;
use crate::lang::r#struct::Struct;
use std::path::Path;
use crate::util::replace::Replace;
use regex::Regex;
use std::collections::HashMap;
use crate::util::glob::Glob;
use chrono::{Duration, Local, DateTime};
use crate::lang::argument::ArgumentType;

pub trait ArgumentVector {
    fn check_len(&self, len: usize) -> CrushResult<()>;
    fn string(&mut self, idx: usize) -> CrushResult<Box<str>>;
    fn integer(&mut self, idx: usize) -> CrushResult<i128>;
    fn field(&mut self, idx: usize) -> CrushResult<Vec<Box<str>>>;
    fn command(&mut self, idx: usize) -> CrushResult<Box<dyn CrushCommand + Send + Sync>>;
    fn r#type(&mut self, idx: usize) -> CrushResult<ValueType>;
    fn value(&mut self, idx: usize) -> CrushResult<Value>;
    fn glob(&mut self, idx: usize) -> CrushResult<Glob>;
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
        Box::from(SimpleCommand { call, can_block })
    }

    pub fn condition(call: fn(context: ExecutionContext) -> CrushResult<()>) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(ConditionCommand { call })
    }
}

#[derive(Clone)]
struct SimpleCommand {
    pub call: fn(context: ExecutionContext) -> CrushResult<()>,
    pub can_block: bool,
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
        Box::from(SimpleCommand { call: self.call, can_block: self.can_block })
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
        Box::from(ConditionCommand { call: self.call })
    }
}

impl std::cmp::PartialEq for ConditionCommand {
    fn eq(&self, _other: &ConditionCommand) -> bool {
        return false;
    }
}

impl std::cmp::Eq for ConditionCommand {}

#[derive(Clone)]
struct Closure {
    job_definitions: Vec<Job>,
    signature: Option<Vec<Parameter>>,
    env: Scope,
}

impl CrushCommand for Closure {
    fn name(&self) -> &str { "closure" }

    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let job_definitions = self.job_definitions.clone();
        let parent_env = self.env.clone();
        let env = parent_env.create_child(&context.env, false);

        if let Some(this) = context.this {
            env.redeclare("this", this)?;
        }
        Closure::push_arguments_to_env(&self.signature, context.arguments, &env)?;

        match job_definitions.len() {
            0 => return error("Empty closures not supported"),
            1 => {
                if env.is_stopped() {
                    return Ok(());
                }
                let job = job_definitions[0].invoke(&env, context.input, context.output)?;
                job.join();
                if env.is_stopped() {
                    return Ok(());
                }
            }
            _ => {
                if env.is_stopped() {
                    return Ok(());
                }
                let first_job_definition = &job_definitions[0];
                let last_output = spawn_print_thread();
                let first_job = first_job_definition.invoke(&env, context.input, last_output)?;
                first_job.join();
                if env.is_stopped() {
                    return Ok(());
                }
                for job_definition in &job_definitions[1..job_definitions.len() - 1] {
                    let last_output = spawn_print_thread();
                    let job = job_definition.invoke(&env, empty_channel(), last_output)?;
                    job.join();
                    if env.is_stopped() {
                        return Ok(());
                    }
                }

                let last_job_definition = &job_definitions[job_definitions.len() - 1];
                let last_job = last_job_definition.invoke(&env, empty_channel(), context.output)?;
                last_job.join();
                if env.is_stopped() {
                    return Ok(());
                }
            }
        }
        Ok(())
    }

    fn can_block(&self, arg: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
        if self.job_definitions.len() == 1 {
            self.job_definitions[0].can_block(env)
        } else {
            true
        }
    }

    fn clone(&self) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(Closure {
            signature: self.signature.clone(),
            job_definitions: self.job_definitions.clone(),
            env: self.env.clone(),
        })
    }
}

impl Closure {
    /*
        pub fn spawn_stream(&self, context: StreamExecutionContext) -> CrushResult<()> {
            let job_definitions = self.job_definitions.clone();
            let parent_env = self.env.clone();
            Ok(())
        }
    */

    fn push_arguments_to_env(
        signature: &Option<Vec<Parameter>>,
        mut arguments: Vec<Argument>,
        env: &Scope) -> CrushResult<()> {
        if let Some(signature) = signature {
            let mut named = HashMap::new();
            let mut unnamed = Vec::new();
            for arg in arguments.drain(..) {
                match arg.argument_type {
                    Some(name) => {
                        named.insert(name.clone(), arg.value);
                    }
                    None => unnamed.push(arg.value),
                };
            }
            let mut unnamed_name = None;
            let mut named_name = None;

            for param in signature {
                match param {
                    Parameter::Parameter(name, value_type, default) => {
                        if let (_, Value::Type(value_type)) = value_type.compile_non_blocking(env)? {
                            if named.contains_key(name.as_ref()) {
                                let value = named.remove(name.as_ref()).unwrap();
                                if !value_type.is(&value) {
                                    return argument_error("Wrong parameter type");
                                }
                                env.redeclare(name.as_ref(), value)?;
                            } else {
                                if let Some(default) = default {
                                    env.redeclare(name.as_ref(), default.compile_non_blocking(env)?.1)?;
                                } else {
                                    return argument_error("Missing variable!!!");
                                }
                            }
                        } else {
                            return argument_error("Not a type");
                        }
                    }
                    Parameter::Named(name) => {
                        if named_name.is_some() {
                            return argument_error("Multiple named argument maps specified");
                        }
                        named_name = Some(name);
                    }
                    Parameter::Unnamed(name) => {
                        if unnamed_name.is_some() {
                            return argument_error("Multiple named argument maps specified");
                        }
                        unnamed_name = Some(name);
                    }
                }
            }

            if let Some(unnamed_name) = unnamed_name {
                env.redeclare(
                    unnamed_name.as_ref(),
                    Value::List(List::new(ValueType::Any, unnamed)))?; } else {
                if !unnamed.is_empty() {
                    return argument_error("No target for unnamed arguments");
                }
            }

            if let Some(named_name) = named_name {
                let d = Dict::new(ValueType::String, ValueType::Any);
                for (k, v) in named {
                    d.insert(Value::String(k), v)?;
                }
                env.redeclare(named_name.as_ref(), Value::Dict(d))?;
            } else {
                if !named.is_empty() {
                    return argument_error("No target for extra named arguments");
                }
            }
        } else {
            for arg in arguments.drain(..) {
                match arg.argument_type {
                    Some(name) => {
                        env.redeclare(name.as_ref(), arg.value)?;
                    },
                    None => {
                        return argument_error("No target for unnamed arguments");
                    },
                }
            }
        }
        Ok(())
    }
}

impl ToString for Closure {
    fn to_string(&self) -> String {
        self.job_definitions.iter().map(|j| j.to_string()).collect::<Vec<String>>().join("; ")
    }
}

#[derive(Clone)]
pub enum Parameter {
    Parameter(Box<str>, ValueDefinition, Option<ValueDefinition>),
    Named(Box<str>),
    Unnamed(Box<str>),
}
