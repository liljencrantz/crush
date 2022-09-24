use crate::lang::argument::Argument;
use crate::lang::command::Command;
use crate::lang::data::dict::Dict;
use crate::lang::data::list::List;
use crate::data::r#struct::Struct;
use crate::lang::state::scope::Scope;
use crate::lang::data::table::Table;
use crate::lang::errors::{argument_error_legacy, CrushResult, error};
use crate::lang::state::global_state::{GlobalState, JobHandle};
use crate::lang::pipe::{
    black_hole, empty_channel, InputStream, OutputStream, ValueReceiver, ValueSender,
};
use crate::lang::printer::Printer;
use crate::lang::value::{Value, ValueType};
use crate::util::glob::Glob;
use crate::util::replace::Replace;
use chrono::{DateTime, Duration, Local};
use regex::Regex;
use std::mem::swap;
use std::path::PathBuf;
use std::thread::ThreadId;

/**
The data needed to be passed around while parsing and compiling code.
 */
pub struct CompileContext {
    pub env: Scope,
    pub global_state: GlobalState,
}

impl CompileContext {
    pub fn new(env: Scope, global_state: GlobalState) -> CompileContext {
        CompileContext { env, global_state }
    }

    pub fn job_context(&self, input: ValueReceiver, output: ValueSender) -> JobContext {
        JobContext::new(input, output, self.env.clone(), self.global_state.clone())
    }

    pub fn with_scope(&self, env: &Scope) -> CompileContext {
        CompileContext {
            env: env.clone(),
            global_state: self.global_state.clone(),
        }
    }
}

impl From<&JobContext> for CompileContext {
    fn from(c: &JobContext) -> Self {
        CompileContext::new(c.scope.clone(), c.global_state.clone())
    }
}

impl From<&CommandContext> for CompileContext {
    fn from(c: &CommandContext) -> Self {
        CompileContext::new(c.scope.clone(), c.global_state.clone())
    }
}

/**
The data needed to be passed around while executing a job.
 */
#[derive(Clone)]
pub struct JobContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub scope: Scope,
    pub global_state: GlobalState,
    pub handle: Option<JobHandle>,
}

impl JobContext {
    pub fn new(
        input: ValueReceiver,
        output: ValueSender,
        env: Scope,
        global_state: GlobalState,
    ) -> JobContext {
        JobContext {
            input,
            output,
            scope: env,
            global_state,
            handle: None,
        }
    }

    pub fn running(&self, desc: String) -> JobContext {
        JobContext {
            input: self.input.clone(),
            output: self.output.clone(),
            scope: self.scope.clone(),
            global_state: self.global_state.clone(),
            handle: Some(self.global_state.job_begin(desc)),
        }
    }

    pub fn with_io(&self, input: ValueReceiver, output: ValueSender) -> JobContext {
        JobContext {
            input,
            output,
            scope: self.scope.clone(),
            global_state: self.global_state.clone(),
            handle: self.handle.clone(),
        }
    }

    pub fn command_context(&self, arguments: Vec<Argument>, this: Option<Value>) -> CommandContext {
        CommandContext {
            arguments,
            this,
            input: self.input.clone(),
            output: self.output.clone(),
            scope: self.scope.clone(),
            global_state: self.global_state.clone(),
            handle: self.handle.clone(),
        }
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
        where
            F: FnOnce() -> CrushResult<()>,
            F: Send + 'static,
    {
        self.global_state.threads().spawn(name, self.handle.clone().map(|h| { h.id() }), f)
    }
}

/**
The data needed to be passed into a command when executing it.
 */
#[derive(Clone)]
pub struct CommandContext {
    pub input: ValueReceiver,
    pub output: ValueSender,
    pub arguments: Vec<Argument>,
    pub scope: Scope,
    pub this: Option<Value>,
    pub global_state: GlobalState,
    handle: Option<JobHandle>,
}

impl CommandContext {
    /**
    Return a new Command context with the same scope and state, but empty I/O and arguments.
     */
    pub fn new(scope: &Scope, state: &GlobalState) -> CommandContext {
        CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            scope: scope.clone(),
            this: None,
            global_state: state.clone(),
            handle: None,
        }
    }

    /**
    Clear the argument vector and return the original.
     */
    pub fn remove_arguments(&mut self) -> Vec<Argument> {
        let mut tmp = Vec::new(); // This does not cause a memory allocation
        swap(&mut self.arguments, &mut tmp);
        tmp
    }

    /**
    Return a new Command context with the same scope and state, but otherwise empty.
     */
    pub fn empty(&self) -> CommandContext {
        CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            scope: self.scope.clone(),
            this: None,
            global_state: self.global_state.clone(),
            handle: self.handle.clone(),
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different argument vector.
     */
    pub fn with_args(self, arguments: Vec<Argument>, this: Option<Value>) -> CommandContext {
        CommandContext {
            input: self.input,
            output: self.output,
            scope: self.scope,
            arguments,
            this,
            global_state: self.global_state,
            handle: self.handle,
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different output sender.
     */
    pub fn with_output(self, sender: ValueSender) -> CommandContext {
        CommandContext {
            input: self.input,
            output: sender,
            scope: self.scope,
            arguments: self.arguments,
            this: self.this,
            global_state: self.global_state,
            handle: self.handle,
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different output sender.
     */
    pub fn with_scope(self, scope: Scope) -> CommandContext {
        CommandContext {
            input: self.input,
            output: self.output,
            scope,
            arguments: self.arguments,
            this: self.this,
            global_state: self.global_state,
            handle: self.handle,
        }
    }

    /**
    Return a new Command context that is identical to this one but with a different input receiver.
     */
    pub fn with_input(self, input: ValueReceiver) -> CommandContext {
        CommandContext {
            input: input,
            output: self.output,
            scope: self.scope,
            arguments: self.arguments,
            this: self.this,
            global_state: self.global_state,
            handle: self.handle.clone(),
        }
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
        where
            F: FnOnce() -> CrushResult<()>,
            F: Send + 'static,
    {
        self.global_state.threads().spawn(name, self.handle.clone().map(|h| { h.id() }), f)
    }
}

