use crate::env::Env;
use crate::commands::{Call, JobJoinHandle, CallDefinition};
use crate::stream::{print, streams, OutputStream, InputStream, spawn_print_thread};
use std::thread;
use crate::data::{JobOutput, CellFnurp, CellDefinition};
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

    pub fn compile(
        &self,
        env: &Env,
        printer: &Printer,
        first_input_type: &Vec<CellFnurp>,
        mut first_input: InputStream,
        last_output: OutputStream,
    ) -> Result<Job, JobError> {
        let mut deps = Vec::new();
        let mut calls = Vec::new();

        let mut input_type = first_input_type.clone();
        let mut input = first_input;

        let last_job_idx = self.commands.len() - 1;
        for call_def in &self.commands[..last_job_idx] {
            let (output, next_input) = streams();
            let call = call_def.compile(env, printer, input_type, input, output, &mut deps)?;

            input_type = call.get_output_type().clone();
            input = next_input;

            calls.push(call);
        }
        let last_call_def = &self.commands[last_job_idx];
        calls.push(last_call_def.compile(env, printer, input_type, input, last_output, &mut deps)?);

        Ok(Job::new(calls, deps, env, printer))
    }
}

pub struct Job {
    commands: Vec<Call>,
    dependencies: Vec<Job>,
    handlers: Vec<JobJoinHandle>,
    env: Env,
    printer: Printer,
    output_type: Vec<CellFnurp>,
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
            handlers: Vec::new(),
            env: env.clone(),
            printer: printer.clone(),
        }
    }

    pub fn take_handlers(&mut self) -> Vec<JobJoinHandle> {
        self.handlers.drain(..).collect()
    }

    pub fn get_output_type(&self) -> &Vec<CellFnurp> {
        return &self.output_type;
    }

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

    pub fn wait(&mut self, printer: &Printer) {
        for h in self.handlers.drain(..) {
            match h.join() {
                Ok(_) => {}
                Err(e) => {
                    printer.job_error(e);
                }
            }
        }
    }
}
