use crate::state::State;
use crate::commands::{Call, Exec, JobResult};
use crate::stream::{print, streams, InputStream};
use std::mem;
use crate::errors::{JobError, error};
use std::thread;
use std::thread::JoinHandle;
use crate::job::JobState::Empty;
use crate::cell::CellType;

#[derive(PartialEq)]
#[derive(Debug)]
pub enum JobState {
    Empty,
    Parsed,
    Spawned,
    Finished,
}

pub struct Job {
    pub state: JobState,
    pub commands: Vec<Call>,
    pub compile_errors: Vec<JobError>,
    pub runtime_errors: Vec<JobError>,
    handlers: Vec<JobResult>,
}

impl Job {
    pub fn new() -> Job {
        Job {
            state: JobState::Empty,
            commands: Vec::new(),
            compile_errors: Vec::new(),
            runtime_errors: Vec::new(),
            handlers: Vec::new(),
        }
    }

    pub fn exec(&mut self, state: &mut State) {
        assert_eq!(self.state, JobState::Parsed);
        if !self.commands.is_empty() && self.compile_errors.is_empty() {
            let (prev_output, mut input) = streams();
            let output_type = self.commands.last().unwrap().get_output_type().clone();
            drop(prev_output);
            for mut c in self.commands.drain(..) {
                let (output, next_input) = streams();
                self.handlers.push(c.execute(state, input, output));
                input = next_input;
            }
            thread::spawn(move || print(input, output_type));
        }
        self.state = JobState::Spawned;
    }

    pub fn wait(&mut self) {
        assert_eq!(self.state, JobState::Spawned);
        for h in self.handlers.drain(..) {
            match h.join() {
                Ok(res) => {}
                Err(e) => {
                    println!("Runtime error: {}", e.message);
                }
            }
        }
        self.state = JobState::Finished;
    }
}
