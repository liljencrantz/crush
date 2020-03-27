use crate::lang::errors::{CrushResult, argument_error, error};
use crate::lang::argument::{Argument, ArgumentDefinition};
use crate::lang::command::{Parameter, CrushCommand, ExecutionContext};
use crate::lang::scope::Scope;
use std::collections::HashMap;
use crate::lang::value::{Value, ValueType};
use crate::lang::list::List;
use crate::lang::dict::Dict;
use crate::lang::job::Job;
use crate::lang::pretty_printer::spawn_print_thread;
use crate::lang::stream::empty_channel;

#[derive(Clone)]
pub struct Closure {
    pub job_definitions: Vec<Job>,
    pub signature: Option<Vec<Parameter>>,
    pub env: Scope,
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

    fn can_block(&self, _arg: &Vec<ArgumentDefinition>, _env: &Scope) -> bool {
        true
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
            let mut v = Vec::new();

            for param in signature {
                match param {
                    Parameter::Parameter(name, value_type, default) => {
                        if let (_, Value::Type(value_type)) = value_type.compile(&mut v, env)? {
                            if named.contains_key(name.as_ref()) {
                                let value = named.remove(name.as_ref()).unwrap();
                                if !value_type.is(&value) {
                                    return argument_error("Wrong parameter type");
                                }
                                env.redeclare(name.as_ref(), value)?;
                            } else {
                                if let Some(default) = default {
                                    env.redeclare(name.as_ref(), default.compile(&mut v, env)?.1)?;
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
