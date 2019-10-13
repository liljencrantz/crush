use crate::state::State;
use crate::commands::{Call, Exec};
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
    Waited,
    Finished,
}

pub struct Job {
    pub state: JobState,
    pub commands: Vec<Call>,
    pub compile_errors: Vec<JobError>,
    pub runtime_errors: Vec<JobError>,
    handlers: Vec<JoinHandle<Result<(), JobError>>>,
    job_output: Option<InputStream>,
    job_output_type: Option<Vec<CellType>>,
}

impl Job {
    pub fn new() -> Job {
        Job {
            state: JobState::Empty,
            commands: Vec::new(),
            compile_errors: Vec::new(),
            runtime_errors: Vec::new(),
            handlers: Vec::new(),
            job_output: None,
            job_output_type: None,
        }
    }

    pub fn to_string(&self) -> String {
        let el: Vec<String> = self.commands.iter()
            .map(|c| String::from(c.get_name()))
            .collect();
        return el.join(" | ");
    }

    pub fn exec(&mut self, state: &mut State) {
        assert_eq!(self.state, JobState::Parsed);
        if !self.commands.is_empty() && self.compile_errors.is_empty() {
            let (prev_output, mut input) = streams();
            self.job_output_type = Some(self.commands.last().unwrap().get_output_type().clone());
            drop(prev_output);
            for mut c in self.commands.drain(..) {
                let (output, next_input) = streams();
                if let Some(h) = c.execute(state, input, output) {
                    self.handlers.push(h);
                }
                input = next_input;
            }
            self.job_output = Some(input);
        }
        self.state = JobState::Spawned;
    }

    pub fn wait(&mut self) {
        assert_eq!(self.state, JobState::Spawned);
        match (self.job_output_type.take(), self.job_output.take()) {
            (Some(types), Some(mut stream)) => {
                print(&mut stream, &types);
            }
            _ => {}
        }
        for h in self.handlers.drain(..) {
            match h.join() {
                Ok(res) => {
                    match res {
                        Ok(_) => {},
                        Err(err) => {
                            self.runtime_errors.push(err);
                        },
                    }
                }
                Err(e) => {
                    self.runtime_errors.push(error("Failed while waiting for command to finish"))
                }
            }
        }
        self.state = JobState::Waited;
    }
}
