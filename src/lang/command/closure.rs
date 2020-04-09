use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::argument::{Argument, ArgumentDefinition};
use crate::lang::command::{Parameter, CrushCommand, BoundCommand};
use crate::lang::scope::Scope;
use std::collections::HashMap;
use crate::lang::value::{Value, ValueType};
use crate::lang::list::List;
use crate::lang::dict::Dict;
use crate::lang::job::Job;
use crate::lang::stream::{empty_channel, black_hole};
use crate::lang::execution_context::{ExecutionContext, CompileContext, JobContext};
use crate::lang::help::Help;
use crate::lang::serialization::SerializationState;
use crate::lang::serialization::model::Element;

pub struct Closure {
    name: Option<Box<str>>,
    job_definitions: Vec<Job>,
    signature: Option<Vec<Parameter>>,
    env: Scope,
    short_help: String,
    long_help: String,
}

impl CrushCommand for Closure {
    fn name(&self) -> &str { "closure" }

    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let job_definitions = self.job_definitions.clone();
        let parent_env = self.env.clone();
        let env = parent_env.create_child(&context.env, false);

        let mut cc = context.compile_context().with_scope(&env);
        if let Some(this) = context.this {
            env.redeclare("this", this)?;
        }
        Closure::push_arguments_to_env(
            &self.signature,
            context.arguments,
            &mut cc)?;

        if env.is_stopped() {
            return Ok(());
        }
        for (idx, job_definition) in job_definitions.iter().enumerate() {
            let first = idx == 0;
            let last = idx == job_definitions.len() - 1;
            let input = if first { context.input.clone() } else { empty_channel() };
            let output = if last { context.output.clone() } else { black_hole() };
            let job = job_definition.invoke(JobContext::new(input, output, env.clone(), context.printer.clone()))?;
            job.join(&context.printer);
            if env.is_stopped() {
                return Ok(());
            }
        }
        Ok(())
    }

    fn can_block(&self, _arg: &Vec<ArgumentDefinition>, _context: &mut CompileContext) -> bool {
        true
    }

    fn clone(&self) -> Box<dyn CrushCommand + Send + Sync> {
        Box::from(Closure {
            name: self.name.clone(),
            signature: self.signature.clone(),
            job_definitions: self.job_definitions.clone(),
            env: self.env.clone(),
            short_help: self.short_help.clone(),
            long_help: self.long_help.clone(),
        })
    }

    fn help(&self) -> &dyn Help {
        self
    }

    fn serialize(&self, _elements: &mut Vec<Element>, _state: &mut SerializationState) -> CrushResult<usize> {
        unimplemented!();
    }

    fn bind(&self, this: Value) -> Box<dyn CrushCommand +  Send + Sync> {
        Box::from(BoundCommand {
            command: self.clone(),
            this,
        })
    }
}

impl Help for Closure {
    fn signature(&self) -> String {
        format!(
            "{} {}",
            self.name.as_ref().unwrap_or(&Box::from("<unnamed>")).to_string(),
            self.signature.as_ref()
                .map(|s| s
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<_>>()
                    .join(" "))
                .unwrap_or("".to_string()),
        )
    }

    fn short_help(&self) -> String {
        self.short_help.clone()
    }

    fn long_help(&self) -> Option<String> {
        Some(self.long_help.clone())
    }
}

fn extract_help(jobs: &mut Vec<Job>) -> String {
    if jobs.len() == 0 {
        return "".to_string();
    }

    let j = &jobs[0];
    match j.as_string() {
        Some(help) => {
            if jobs.len() > 1 {
                jobs.remove(0);
            }
            help
        }
        _ => "".to_string()
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

    pub fn new(
        name: Option<Box<str>>,
        signature: Option<Vec<Parameter>>,
        mut job_definitions: Vec<Job>,
        env: Scope,
    ) -> Closure {
        let short_help = extract_help(&mut job_definitions);
        let long_help = extract_help(&mut job_definitions);

        Closure {
            name,
            job_definitions,
            signature,
            env,
            short_help,
            long_help,
        }
    }

    fn push_arguments_to_env(
        signature: &Option<Vec<Parameter>>,
        mut arguments: Vec<Argument>,
        context: &mut CompileContext) -> CrushResult<()> {
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
                        if let Value::Type(value_type) = value_type.compile_bound(context)? {
                            if named.contains_key(name.as_ref()) {
                                let value = named.remove(name.as_ref()).unwrap();
                                if !value_type.is(&value) {
                                    return argument_error("Wrong parameter type");
                                }
                                context.env.redeclare(name.as_ref(), value)?;
                            } else {
                                if unnamed.len() > 0 {
                                    context.env.redeclare(name.as_ref(), unnamed.remove(0))?;
                                } else {
                                    if let Some(default) = default {
                                        let env = context.env.clone();
                                        env.redeclare(name.as_ref(), default.compile_bound(context)?)?;
                                    } else {
                                        return argument_error("Missing variable!!!");
                                    }
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
                context.env.redeclare(
                    unnamed_name.as_ref(),
                    Value::List(List::new(ValueType::Any, unnamed)))?;
            } else {
                if !unnamed.is_empty() {
                    return argument_error("No target for unnamed arguments");
                }
            }

            if let Some(named_name) = named_name {
                let d = Dict::new(ValueType::String, ValueType::Any);
                for (k, v) in named {
                    d.insert(Value::string(&k), v)?;
                }
                context.env.redeclare(named_name.as_ref(), Value::Dict(d))?;
            } else {
                if !named.is_empty() {
                    return argument_error("No target for extra named arguments");
                }
            }
        } else {
            for arg in arguments.drain(..) {
                match arg.argument_type {
                    Some(name) => {
                        context.env.redeclare(name.as_ref(), arg.value)?;
                    }
                    None => {
                        return argument_error("No target for unnamed arguments");
                    }
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
