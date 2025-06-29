use crate::lang::ast::location::Location;
/// An executable pipeline of one or more commands.
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::{CompileContext, JobContext};
use std::fmt::{Display, Formatter};
use std::thread::ThreadId;

/// An executable pipeline of one or more commands.
#[derive(Clone)]
pub struct Job {
    commands: Vec<CommandInvocation>,
    location: Location,
}

impl Job {
    pub fn location(&self) -> Location {
        self.location
    }

    pub fn new(commands: Vec<CommandInvocation>, location: Location) -> Job {
        Job { commands, location }
    }

    pub fn can_block(&self, context: &mut CompileContext) -> bool {
        if self.commands.len() == 1 {
            self.commands[0].can_block(context)
        } else {
            true
        }
    }

    pub fn commands(&self) -> &[CommandInvocation] {
        &self.commands
    }

    /// Evaluate this job in the specified context
    pub fn eval(&self, context: JobContext) -> CrushResult<Option<ThreadId>> {
        let context = context.running(self.to_string());
        let mut input = context.input.clone();
        let last_job_idx = self.commands.len() - 1;
        for call_def in &self.commands[..last_job_idx] {
            let (output, next_input) = pipe();
            call_def.eval(context.with_io(input, output))?;
            input = next_input;

            if context.scope.is_stopped() {
                return Ok(None);
            }
        }

        if context.scope.is_stopped() {
            return Ok(None);
        }

        let last_call_def = &self.commands[last_job_idx];
        last_call_def
            .eval(context.with_io(input, context.output.clone()))
            .map_err(|e| e.with_location(self.location))
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
