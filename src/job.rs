use crate::state::State;
use crate::commands::Call;
use crate::stream::{print, streams};
use std::mem;
use crate::errors::{JobError, error};
use std::thread;
use std::thread::JoinHandle;
use crate::job::JobState::Empty;

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
    pub handlers: Vec<Option<JoinHandle<Result<(), JobError>>>>,
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

    pub fn to_string(&self) -> String {
        let el: Vec<String> = self.commands.iter()
            .map(|c| String::from(c.get_name()))
            .collect();
        return el.join(" | ");
    }

    pub fn spawn(&mut self, state: &State) {
        assert_eq!(self.state, JobState::Parsed);
        self.state = JobState::Spawned;
        if !self.commands.is_empty() && self.compile_errors.is_empty() {
            let (prev_output, mut prev_input) = streams();
            drop(prev_output);
            for c in &mut self.commands {
                let (mut output, input) = streams();

                let mut cc = c.clone();
                self.handlers.push(Some(thread::spawn(move || {
                    return cc.run(&mut prev_input, &mut output);
                })));
                prev_input = input;
            }
            match self.commands.last() {
                Some(command) => print(&mut prev_input, command.get_output_type()),
                None => {}
            }
        }
    }

    pub fn wait(&mut self) {
        assert_eq!(self.state, JobState::Spawned);
        for h in &mut self.handlers {
            match h.take().unwrap().join() {
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

    pub fn mutate(&mut self, state: &mut State) -> Result<(), ()> {
        assert_eq!(self.state, JobState::Waited);
        if !self.commands.is_empty() && self.compile_errors.is_empty() && self.runtime_errors.is_empty() {
            for c in &mut self.commands {
                match c.mutate(state) {
                    Ok(_) => {}
                    Err(err) => {
                        self.runtime_errors.push(err);
                        break;
                    }
                }
            }
        }
        self.state = JobState::Finished;
        return if self.runtime_errors.is_empty() { Ok(()) } else { Err(()) };
    }
}
