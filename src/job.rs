use crate::state::State;
use crate::commands::{Call, JobResult, CallDefinition};
use crate::stream::{print, streams, OutputStream};
use std::thread;
use crate::data::{Output, CellDefinition};
use std::thread::JoinHandle;
use crate::printer::Printer;
use map_in_place::MapVecInPlace;

#[derive(PartialEq)]
#[derive(Debug)]
pub enum JobState {
    Parsed,
    Spawned,
    Finished,
}

#[derive(Clone)]
pub struct JobDefinition {
    commands: Vec<CallDefinition>,
}

impl JobDefinition {
    pub fn new(commands: Vec<CallDefinition>) -> JobDefinition {
        JobDefinition { commands }
    }

    pub fn job(&self) -> Job {
        let mut deps = Vec::new();
        let mut jobs = Vec::new();
        let mut input_type = Vec::new();
        for def in &self.commands {
            let c = def.call(input_type, &mut deps);
            input_type = c.get_input_type().clone();
            jobs.push(c);
        }
        Job::new(
            jobs, deps)
    }
}

pub struct Job {
    state: JobState,
    commands: Vec<Call>,
    dependencies: Vec<Job>,
    handlers: Vec<JobResult>,
    output: Option<Output>,
    last_output_stream: Option<OutputStream>,
    print_thread: Option<JoinHandle<()>>,
}

impl Job {
    pub fn new(commands: Vec<Call>, dependencies: Vec<Job>) -> Job {
        let (last_output_stream, last_input_stream) = streams();
        let last = commands.last().unwrap();
        let output = Some(Output { types: last.get_output_type().clone(), stream: last_input_stream });
        Job {
            state: JobState::Parsed,
            commands,
            dependencies,
            handlers: Vec::new(),
            output,
            last_output_stream: Some(last_output_stream),
            print_thread: None,
        }
    }

    pub fn take_output(&mut self) -> Option<Output> {
        self.output.take()
    }

    pub fn exec(&mut self, state: &mut State) {
        assert_eq!(self.state, JobState::Parsed);

        for dep in self.dependencies.iter_mut() {
            dep.exec(state);
        }
        if !self.commands.is_empty() {
            let (prev_output, mut input) = streams();
            drop(prev_output);
            let last_job_idx = self.commands.len() - 1;
            for c in self.commands.drain(..last_job_idx) {
                let (output, next_input) = streams();
                self.handlers.push(c.execute(state, input, output));
                input = next_input;
            }
            let last_command = self.commands.drain(..).next().unwrap();
            self.handlers.push(last_command.execute(state, input, self.last_output_stream.take().unwrap()));
        }
        self.state = JobState::Spawned;
    }

    pub fn print(&mut self, printer: &Printer) {
        let p = printer.clone();
        if let Some(output) = self.take_output() {
            self.print_thread = Some(
                thread::Builder::new()
                    .name("output_formater".to_string())
                    .spawn(move || print(&p, output.stream, output.types)
                    ).unwrap()
            );
        }
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
        self.print_thread.take().map(|h| h.join());
        self.state = JobState::Finished;
    }
}
