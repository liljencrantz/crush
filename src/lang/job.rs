use crate::lang::command_invocation::CommandInvocation;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::{CompileContext, JobContext};
use crate::lang::printer::Printer;
use crate::lang::stream::channels;
use std::thread::JoinHandle;

pub enum JobJoinHandle {
    Many(Vec<JobJoinHandle>),
    Async(JoinHandle<()>),
}

impl JobJoinHandle {
    pub fn join(self, printer: &Printer) {
        match self {
            JobJoinHandle::Async(a) => match a.join() {
                Ok(_) => {}
                Err(_) => printer.error("Unknown error while waiting for command to exit"),
            },
            JobJoinHandle::Many(v) => {
                for j in v {
                    j.join(printer);
                }
            }
        }
    }
}

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

    pub fn invoke(&self, context: JobContext) -> CrushResult<JobJoinHandle> {
        let mut calls = Vec::new();

        let mut input = context.input.clone();
        let last_job_idx = self.commands.len() - 1;
        for call_def in &self.commands[..last_job_idx] {
            let (output, next_input) = channels();
            let call = call_def.invoke(context.with_io(input, output))?;
            input = next_input;
            calls.push(call);
        }
        let last_call_def = &self.commands[last_job_idx];
        calls.push(last_call_def.invoke(context.with_io(input, context.output.clone()))?);

        Ok(JobJoinHandle::Many(calls))
    }

    pub fn as_string(&self) -> Option<String> {
        if self.commands.len() != 1 {
            return None;
        }

        self.commands[0].as_string()
    }
}

impl ToString for Job {
    fn to_string(&self) -> String {
        self.commands
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<String>>()
            .join("|")
    }
}
