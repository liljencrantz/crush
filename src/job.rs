use crate::env::Env;
use crate::commands::{Call, JobResult, CallDefinition};
use crate::stream::{print, streams, OutputStream, InputStream};
use std::thread;
use crate::data::{Output, CellDefinition, CellType};
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

    pub fn compile(&self, env: &Env, printer: &Printer, initial_input_type: &Vec<CellType>, input: InputStream, output: OutputStream) -> Result<Job, JobError> {
        let mut deps = Vec::new();
        let mut jobs = Vec::new();
        let mut input_type = initial_input_type.clone();
        for def in &self.commands {
            let c = def.compile(input_type, &mut deps, env, printer)?;
            input_type = c.get_output_type().clone();
            jobs.push(c);
        }
        Ok(Job::new(
            jobs, deps, input, output, env, printer))
    }
}

pub struct Job {
    commands: Vec<Call>,
    dependencies: Vec<Job>,
    handlers: Vec<JobResult>,
    first_input: Option<InputStream>,
    last_output: Option<OutputStream>,
    output_type: Vec<CellType>,
    env: Env,
    printer: Printer,
}

impl Job {
    pub fn new(
        commands: Vec<Call>,
        dependencies: Vec<Job>,
        first_input: InputStream,
        last_output: OutputStream,
    env: &Env,
    printer: &Printer) -> Job {
        Job {
            output_type: commands[commands.len()-1].get_output_type().clone(),
            commands,
            dependencies,
            handlers: Vec::new(),
            first_input: Some(first_input),
            last_output: Some(last_output),
            env: env.clone(),
            printer: printer.clone(),
        }
    }

    pub fn take_handlers(&mut self) -> Vec<JobResult> {
        self.handlers.drain(..).collect()
    }

    pub fn get_output_type(&self) -> &Vec<CellType> {
        return &self.output_type;
    }

    pub fn exec(&mut self) {
        for dep in self.dependencies.iter_mut() {
            dep.exec();
        }
        if !self.commands.is_empty() {
            let mut input = self.first_input.take().unwrap();
            let last_job_idx = self.commands.len() - 1;
            for c in self.commands.drain(..last_job_idx) {
                let (output, next_input) = streams();
                self.handlers.push(c.execute(&self.env, &self.printer, input, output));
                input = next_input;
            }
            let last_command = self.commands.drain(..).next().unwrap();
            self.handlers.push(last_command.execute(&self.env, &self.printer, input, self.last_output.take().unwrap()));
        }
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
