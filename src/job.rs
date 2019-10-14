use crate::state::State;
use crate::commands::{Call, JobResult};
use crate::stream::{print, streams, OutputStream};
use std::thread;
use crate::cell::{Output};

#[derive(PartialEq)]
#[derive(Debug)]
pub enum JobState {
    Parsed,
    Spawned,
    Finished,
}

pub struct Job {
    state: JobState,
    commands: Vec<Call>,
    dependencies: Vec<Job>,
    handlers: Vec<JobResult>,
    output: Option<Output>,
    last_output_stream: Option<OutputStream>,
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

    pub fn print(&mut self) {
        if let Some(output) = self.take_output() {
            thread::spawn(move || print(output.stream, output.types));
        }
    }

    pub fn wait(&mut self) {
        assert_eq!(self.state, JobState::Spawned);
        for h in self.handlers.drain(..) {
            match h.join() {
                Ok(_) => {}
                Err(e) => {
                    println!("Runtime error: {}", e.message);
                }
            }
        }
        self.state = JobState::Finished;
    }
}
