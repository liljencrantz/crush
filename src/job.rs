use crate::env::Env;
use crate::commands::{Call, JobJoinHandle};
use crate::stream::{print, streams, OutputStream, InputStream, spawn_print_thread, UninitializedOutputStream, UninitializedInputStream};
use std::thread;
use crate::data::{JobOutput, ColumnType, CellDefinition, CallDefinition};
use std::thread::JoinHandle;
use crate::printer::Printer;
use map_in_place::MapVecInPlace;
use crate::errors::JobError;

#[derive(Clone)]
#[derive(PartialEq)]
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
        mut first_input: UninitializedInputStream,
        last_output: UninitializedOutputStream,
    ) -> Result<JobJoinHandle, JobError> {
        let mut deps = Vec::new();
        let mut calls = Vec::new();

        let mut input = first_input;

        let last_job_idx = self.commands.len() - 1;
        for call_def in &self.commands[..last_job_idx] {
            let (output, next_input) = streams();
            let call = call_def.spawn_and_execute(env, printer, input, output, &mut deps)?;
            input = next_input;
            calls.push(call);
        }
        let last_call_def = &self.commands[last_job_idx];
        calls.push(last_call_def.spawn_and_execute(env, printer, input, last_output, &mut deps)?);

        Ok(JobJoinHandle::Many(calls))
    }
}

pub struct Job {
    commands: Vec<Call>,
    dependencies: Vec<Job>,
    env: Env,
    printer: Printer,
    output_type: Vec<ColumnType>,
}

impl Job {
    pub fn new(
        commands: Vec<Call>,
        dependencies: Vec<Job>,
        env: &Env,
        printer: &Printer,
    ) -> Job {
        Job {
            output_type: commands[commands.len()-1].get_output_type().clone(),
            commands,
            dependencies,
            env: env.clone(),
            printer: printer.clone(),
        }
    }

    pub fn get_output_type(&self) -> &Vec<ColumnType> {
        return &self.output_type;
    }
/*
    pub fn execute(&mut self) -> JobJoinHandle {
        for mut dep in self.dependencies.drain(..) {
            dep.execute();
        }
        let mut res: Vec<JobJoinHandle> = Vec::new();
        for mut call in self.commands.drain(..) {
            res.push(call.execute());
        }
        JobJoinHandle::Many(res)
    }
*/
}
