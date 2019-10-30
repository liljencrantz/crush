use crate::env::Env;
use crate::commands::{JobJoinHandle};
use crate::stream::{streams, UninitializedOutputStream, UninitializedInputStream};
use crate::data::CallDefinition;
use crate::printer::Printer;
use crate::errors::JobError;

#[derive(Clone)]
pub struct JobDefinition {
    commands: Vec<CallDefinition>,
}

impl JobDefinition {
    pub fn new(commands: Vec<CallDefinition>) -> JobDefinition {
        JobDefinition { commands }
    }

    pub fn spawn_and_execute(
        &self,
        env: &Env,
        printer: &Printer,
        first_input: UninitializedInputStream,
        last_output: UninitializedOutputStream,
    ) -> Result<JobJoinHandle, JobError> {
        let mut calls = Vec::new();

        let mut input = first_input;

        let last_job_idx = self.commands.len() - 1;
        for call_def in &self.commands[..last_job_idx] {
            let (output, next_input) = streams();
            let call = call_def.spawn_and_execute(env, printer, input, output)?;
            input = next_input;
            calls.push(call);
        }
        let last_call_def = &self.commands[last_job_idx];
        calls.push(last_call_def.spawn_and_execute(env, printer, input, last_output)?);

        Ok(JobJoinHandle::Many(calls))
    }
}

