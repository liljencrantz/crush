use crate::lang::errors::{CrushResult, error};
use std::fmt::Formatter;
use crate::lang::stream::{ValueReceiver, ValueSender, InputStream, empty_channel};
use crate::lang::{argument::Argument, argument::ArgumentDefinition};
use crate::lang::scope::Scope;
use crate::lang::job::Job;
use crate::lang::stream_printer::spawn_print_thread;
use crate::lang::value::Value;

pub struct ExecutionContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub env: Scope,
    pub this: Option<Value>,
}

pub struct StreamExecutionContext {
    pub argument_stream: InputStream,
    pub output: ValueSender,
    pub env: Scope,
}

pub trait CrushCommand {
    fn invoke(&self, context: ExecutionContext) -> CrushResult<()>;
    fn can_block(&self, arguments: &Vec<ArgumentDefinition>, env: &Scope) -> bool;
    fn name(&self) -> &str;
}

#[derive(Clone)]
pub struct SimpleCommand {
    pub call: fn(context: ExecutionContext) -> CrushResult<()>,
    pub can_block: bool,
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

    fn name(&self) -> &str {"command"}

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

    fn name(&self) -> &str {"conditional command"}

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

#[derive(Clone)]
#[derive(Debug)]
pub struct Closure {
    job_definitions: Vec<Job>,
    env: Scope,
}

impl CrushCommand for Closure {
    fn name(&self) -> &str {"closure"}

    fn invoke(&self, context: ExecutionContext) -> CrushResult<()> {
        let job_definitions = self.job_definitions.clone();
        let parent_env = self.env.clone();
        let env = parent_env.create_child(&context.env, false);

        if let Some(this) = context.this {
            env.redeclare("this", this);
        }
        Closure::push_arguments_to_env(context.arguments, &env);

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
                    let job = job_definition.invoke(&env,  empty_channel(), last_output)?;
                    job.join();
                    if env.is_stopped() {
                        return Ok(());
                    }
                }

                let last_job_definition = &job_definitions[job_definitions.len() - 1];
                let last_job = last_job_definition.invoke(&env,  empty_channel(), context.output)?;
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
}

impl Closure {
    pub fn new(job_definitions: Vec<Job>, env: &Scope) -> Closure {
        Closure {
            job_definitions,
            env: env.clone(),
        }
    }
    /*
        pub fn spawn_stream(&self, context: StreamExecutionContext) -> CrushResult<()> {
            let job_definitions = self.job_definitions.clone();
            let parent_env = self.env.clone();
            Ok(())
        }
    */

    fn push_arguments_to_env(mut arguments: Vec<Argument>, env: &Scope) {
        for arg in arguments.drain(..) {
            if let Some(name) = &arg.name {
                env.redeclare(name.as_ref(), arg.value);
            }
        }
    }
}

impl ToString for Closure {
    fn to_string(&self) -> String {
        self.job_definitions.iter().map(|j| j.to_string()).collect::<Vec<String>>().join("; ")
    }
}
