use crate::state::State;
use crate::commands::{Call, JobResult, CallDefinition};
use crate::stream::{print, streams, OutputStream, InputStream};
use std::thread;
use crate::data::{Output, CellDefinition, CellType};
use std::thread::JoinHandle;
use crate::printer::Printer;
use map_in_place::MapVecInPlace;
use crate::errors::JobError;

#[derive(PartialEq)]
#[derive(Debug)]
pub enum JobState {
    Parsed,
    Spawned,
    Finished,
}

#[derive(Clone)]
#[derive(PartialEq)]
pub struct JobDefinition {
    commands: Vec<CallDefinition>,
}

impl JobDefinition {
    pub fn new(commands: Vec<CallDefinition>) -> JobDefinition {
        JobDefinition { commands }
    }

    pub fn compile(&self, state: &State, initial_input_type: &Vec<CellType>, input: InputStream, output: OutputStream) -> Result<Job, JobError> {
        let mut deps = Vec::new();
        let mut jobs = Vec::new();
        let mut input_type = initial_input_type.clone();
        for def in &self.commands {
            let c = def.compile(input_type, &mut deps, state)?;
            input_type = c.get_output_type().clone();
            jobs.push(c);
        }
        Ok(Job::new(
            jobs, deps, input, output))
    }
}

pub struct Job {
    state: JobState,
    commands: Vec<Call>,
    dependencies: Vec<Job>,
    handlers: Vec<JobResult>,
    first_input: Option<InputStream>,
    last_output: Option<OutputStream>,
    output_type: Vec<CellType>,
}

impl Job {
    pub fn new(
        commands: Vec<Call>,
        dependencies: Vec<Job>,
        first_input: InputStream,
        last_output: OutputStream) -> Job {
        Job {
            output_type: commands[commands.len()-1].get_output_type().clone(),
            state: JobState::Parsed,
            commands,
            dependencies,
            handlers: Vec::new(),
            first_input: Some(first_input),
            last_output: Some(last_output),
        }
    }

    pub fn take_handlers(&mut self) -> Vec<JobResult> {
        self.handlers.drain(..).collect()
    }

    pub fn get_output_type(&self) -> &Vec<CellType> {
        return &self.output_type;
    }

    pub fn print(printer: &Printer, output: Output) {
        let p = printer.clone();
        thread::Builder::new()
            .name("output_formater".to_string())
            .spawn(move || print(&p, output.stream, output.types)
            );
    }

    pub fn exec(&mut self, state: &mut State, printer: &Printer) {
        assert_eq!(self.state, JobState::Parsed);

        for dep in self.dependencies.iter_mut() {
            dep.exec(state, printer);
        }
        if !self.commands.is_empty() {
            let mut input = self.first_input.take().unwrap();
            let last_job_idx = self.commands.len() - 1;
            for c in self.commands.drain(..last_job_idx) {
                let (output, next_input) = streams();
                self.handlers.push(c.execute(state, printer, input, output));
                input = next_input;
            }
            let last_command = self.commands.drain(..).next().unwrap();
            self.handlers.push(last_command.execute(state, printer, input, self.last_output.take().unwrap()));
        }
        self.state = JobState::Spawned;
    }

    pub fn wait(&mut self, printer: &Printer) {
        assert_eq!(self.state, JobState::Spawned);
        for h in self.handlers.drain(..) {
            match h.join() {
                Ok(_) => {}
                Err(e) => {
                    printer.job_error(e);
                }
            }
        }
        self.state = JobState::Finished;
    }
}
