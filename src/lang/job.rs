use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::{CompileContext, JobContext};
use crate::lang::printer::Printer;
use crate::lang::stream::channels;
use std::thread::{JoinHandle, ThreadId};
use std::fmt::{Display, Formatter};

#[derive(Clone)]
pub struct Job {
    commands: Vec<CommandInvocation>,
}

impl Job {
    pub fn new(commands: Vec<CommandInvocation>) -> Job {
        Job { commands }
    }

    pub fn can_block(&self, context: &mut CompileContext) -> bool {
        if self.commands.len() == 1 {
            self.commands[0].can_block(self.commands[0].arguments(), context)
        } else {
            true
        }
    }

    pub fn commands(&self) -> &[CommandInvocation] {
        &self.commands
    }

    pub fn invoke(&self, context: JobContext) -> CrushResult<Option<ThreadId>> {
        let mut input = context.input.clone();
        let last_job_idx = self.commands.len() - 1;
        for call_def in &self.commands[..last_job_idx] {
            let (output, next_input) = channels();
            call_def.invoke(context.with_io(input, output))?;
            input = next_input;
        }
        let last_call_def = &self.commands[last_job_idx];
        last_call_def.invoke(context.with_io(input, context.output.clone()))
    }

    pub fn as_string(&self) -> Option<String> {
        if self.commands.len() != 1 {
            return None;
        }

        self.commands[0].as_string()
    }
}

impl Display for Job {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for c in self.commands.iter() {
            if first {
                first = false;
            } else {
                f.write_str("|")?;
            }
            c.fmt(f)?;
        }
        Ok(())
    }
}
