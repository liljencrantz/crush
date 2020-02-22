use crate::scope::Scope;
use crate::stream::{channels, ValueSender, ValueReceiver};
use crate::lang::CallDefinition;
use crate::printer::Printer;
use crate::errors::{CrushError, CrushResult};
use std::thread::JoinHandle;

pub enum JobJoinHandle {
    Many(Vec<JobJoinHandle>),
    Async(JoinHandle<CrushResult<()>>),
}

impl JobJoinHandle {
    pub fn join(self, printer: &Printer) {
        return match self {
            JobJoinHandle::Async(a) => match a.join() {
                Ok(r) => match r {
                    Ok(_) => {}
                    Err(e) => printer.job_error(e),
                },
                Err(_) => printer.error("Unknown error while waiting for command to exit"),
            },
            JobJoinHandle::Many(v) => {
                for j in v {
                    j.join(printer);
                }
            }
        };
    }
}

#[derive(Clone)]
#[derive(Debug)]
pub struct Job {
    commands: Vec<CallDefinition>,
}

impl Job {
    pub fn new(commands: Vec<CallDefinition>) -> Job {
        Job { commands }
    }

    pub fn spawn_and_execute(
        &self,
        env: &Scope,
        printer: &Printer,
        first_input: ValueReceiver,
        last_output: ValueSender,
    ) -> Result<JobJoinHandle, CrushError> {
        let mut calls = Vec::new();

        let mut input = first_input;

        let last_job_idx = self.commands.len() - 1;
        for call_def in &self.commands[..last_job_idx] {
            let (output, next_input) = channels();
            let call = call_def.spawn_and_execute(env, printer, input, output)?;
            input = next_input;
            calls.push(call);
        }
        let last_call_def = &self.commands[last_job_idx];
        calls.push(last_call_def.spawn_and_execute(env, printer, input, last_output)?);

        Ok(JobJoinHandle::Many(calls))
    }
}

impl ToString for Job {
    fn to_string(&self) -> String {
        self.commands.iter().map(|c| c.to_string()).collect::<Vec<String>>().join("|")
    }
}
