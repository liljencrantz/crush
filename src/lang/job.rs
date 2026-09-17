use crate::lang::ast::location::Location;
use crate::lang::ast::source::Source;
/// An executable pipeline of one or more commands.
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::{last_element, pipe};
use crate::lang::state::contexts::{EvalContext, JobContext};
use crate::lang::value::Value;
use std::fmt::{Display, Formatter};
use std::thread::ThreadId;

/// An executable pipeline of one or more commands.
#[derive(Clone)]
pub struct Job {
    commands: Vec<CommandInvocation>,
    source: Source,
    /// True only for a job written with a trailing `&` (e.g. `seq 100 | pipe:write &`).
    /// A background job is its own independent execution context: `eval()` dispatches
    /// its real last stage same as any other job, but returns immediately rather than
    /// waiting on it (or any earlier stage) to finish, registering its eventual result
    /// for later retrieval via `fg`. See `eval()`.
    is_background: bool,
}

impl Job {
    pub fn location(&self) -> Location {
        self.source.location()
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    pub fn new(commands: Vec<CommandInvocation>, source: Source, is_background: bool) -> Job {
        Job {
            commands,
            source,
            is_background,
        }
    }

    pub fn can_block(&self, context: &mut EvalContext) -> bool {
        if self.commands.len() == 1 {
            self.commands[0].can_block(context)
        } else {
            true
        }
    }

    pub fn commands(&self) -> &[CommandInvocation] {
        &self.commands
    }

    pub fn is_background(&self) -> bool {
        self.is_background
    }

    /// Evaluate this job in the specified context
    pub fn eval(&self, context: JobContext) -> CrushResult<Option<ThreadId>> {
        context.set_name(self.to_string());
        let mut input = context.input.clone();
        let last_command_idx = self.commands.len() - 1;
        let mut pending_threads = Vec::new();
        for call_def in self.commands[..last_command_idx].iter() {
            let (output, next_input) = pipe();
            match call_def.eval(context.with_io(input, output)) {
                Ok(Some(id)) => pending_threads.push(id),
                Ok(None) => {}
                // Same as the join loop below: a stage running synchronously in this
                // thread (rather than its own spawned one) can still hit a SendError
                // while writing to a downstream stage that already stopped reading
                // (e.g. `head`/`take` truncating the stream) -- benign, not a real
                // failure.
                Err(e) if e.is_send_disconnected() => return Ok(None),
                Err(e) => return Err(e),
            }
            input = next_input;

            if context.scope.is_stopped() {
                return Ok(None);
            }
        }

        if context.scope.is_stopped() {
            return Ok(None);
        }

        let last_call_def = &self.commands[last_command_idx];

        if self.is_background {
            // A background job is its own independent execution context: dispatch the
            // pipeline's real last stage exactly as normal, but don't wait for it --
            // or any earlier stage -- to actually finish; that's the entire point of
            // `&`. `last_call_def.eval(...)?` only ever reports a *synchronous* failure
            // to even start (e.g. a bad argument); the real work, and any failure in
            // it or in an earlier stage, is deliberately left running unjoined. Its
            // eventual result goes to the background-job registry instead, for later
            // retrieval via `fg`, and this job returns immediately.
            let (last_output, last_input) = last_element();
            last_call_def.eval(context.with_io(input, last_output))?;
            let job_id = context.handle.id();
            context.global_state.add_background_job(job_id, last_input);
            context.output.send(Value::from(job_id))?;
            return Ok(None);
        }

        let (last_output, last_input) = if context.output.is_pipeline() {
            pipe()
        } else {
            last_element()
        };
        let res = last_call_def.eval(context.with_io(input, last_output));
        if let Ok(v) = last_input.recv() {
            context.output.send(v)?;
        }

        // Join every non-last stage's thread now that the pipeline has finished, so a
        // genuine failure (not just the expected "downstream stopped reading early",
        // e.g. a `head`/`take` truncating a stream) surfaces as this job's own failure
        // instead of vanishing on a thread nobody ever joined. Joined after computing
        // `res`/forwarding the last stage's output, not before, so pipeline stages
        // still run concurrently rather than blocking on each other in sequence.
        for id in pending_threads {
            if let Err(e) = context.global_state.threads().join_one(id) {
                if !e.is_send_disconnected() {
                    return Err(e);
                }
            }
        }

        res
    }
}

impl Display for Job {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for c in self.commands.iter() {
            if first {
                first = false;
            } else {
                f.write_str(" | ")?;
            }
            c.fmt(f)?;
        }
        Ok(())
    }
}
