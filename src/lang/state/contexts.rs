use crate::lang::argument::Argument;
use crate::lang::ast::source::Source;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::{Stream, ValueReceiver, ValueSender, black_hole, empty_channel, TableOutputStream};
use crate::lang::state::global_state::{CommandHandle, GlobalState, JobHandle};
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use std::mem::swap;
use std::thread::ThreadId;
use crate::lang::data::table::ColumnType;

/**
The data needed to be passed around while calling eval on a ValueDefinition.
 */
pub struct EvalContext {
    pub env: Scope,
    pub global_state: GlobalState,
}

impl EvalContext {
    pub fn new(env: Scope, global_state: GlobalState) -> EvalContext {
        EvalContext { env, global_state }
    }

    pub fn job_context(&self, input: ValueReceiver, output: ValueSender) -> JobContext {
        JobContext::new(input, output, self.env.clone(), self.global_state.clone())
    }

    pub fn with_scope(&self, env: &Scope) -> EvalContext {
        EvalContext {
            env: env.clone(),
            global_state: self.global_state.clone(),
        }
    }
}

impl From<&JobContext> for EvalContext {
    fn from(c: &JobContext) -> Self {
        EvalContext::new(c.scope.clone(), c.global_state.clone())
    }
}

impl From<&CommandContext> for EvalContext {
    fn from(c: &CommandContext) -> Self {
        EvalContext::new(c.scope.clone(), c.global_state.clone())
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
    pub handle: JobHandle,
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
            handle: global_state.create_job_handle(),
            global_state,
        }
    }

    pub fn set_name(&self, name: impl Into<String>) {
        self.handle.set_name(name);
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

    pub fn command_context(
        &self,
        source: &Source,
        arguments: Vec<Argument>,
        this: Option<Value>,
    ) -> CrushResult<CommandContext> {
        Ok(CommandContext {
            arguments,
            this,
            input: self.input.clone(),
            output: self.output.clone(),
            scope: self.scope.clone(),
            handle: self.handle.next_command_handle(),
            global_state: self.global_state.clone(),
            source: source.clone(),
        })
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
    where
        F: FnOnce() -> CrushResult<()>,
        F: Send + 'static,
    {
        self.global_state
            .threads()
            .spawn(name, Some(self.handle.clone()), f)
    }

    pub fn next_command_handle(&self) -> CommandHandle {
        self.handle.next_command_handle()
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
    pub source: Source,
    handle: CommandHandle,
}

impl CommandContext {
    /**
    Return an empty new Command context with the specified scope and state.
     */
    pub fn new(
        scope: &Scope,
        state: &GlobalState,
        source: &Source,
        id: CommandHandle,
    ) -> CommandContext {
        CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            scope: scope.clone(),
            this: None,
            global_state: state.clone(),
            source: source.clone(),
            handle: id,
        }
    }

    /**
    Clear the argument vector and return the original.

    This is useful when you want to parse the argument vector without consuming the whole context.
     */
    pub fn remove_arguments(&mut self) -> Vec<Argument> {
        let mut tmp = Vec::new(); // This does not cause a memory allocation
        swap(&mut self.arguments, &mut tmp);
        tmp
    }

    /**
    Return a new Command context with the same source, scope and state, but otherwise empty.
     */
    pub fn empty(&self) -> CommandContext {
        CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments: Vec::new(),
            scope: self.scope.clone(),
            this: None,
            source: self.source.clone(),
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
            source: self.source,
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
            source: self.source,
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
            source: self.source,
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
            handle: self.handle,
            source: self.source,
        }
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
    where
        F: FnOnce() -> CrushResult<()>,
        F: Send + 'static,
    {
        self.global_state
            .threads()
            .spawn(name, Some(self.handle.job_id.clone()), f)
    }

    pub fn command_handle(&self) -> &CommandHandle {
        &self.handle
    }

    pub fn next_command_handle(&self) -> CommandHandle {
        self.handle.job_id.next_command_handle()
    }

    pub fn input_stream(&self) -> CrushResult<Stream> {
        self.input.recv()?.stream(&self.handle)
    }

    pub fn initialize_output(&self, types: &[ColumnType]) -> CrushResult<TableOutputStream> {
        let (interruptible, controller) = self.output.initialize(types)?.interruptible();
        self.handle.register(controller);
        Ok(interruptible)
    }
}
