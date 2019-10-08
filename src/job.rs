use crate::state::State;
use crate::commands::Call;
use crate::stream::{print, streams};
use std::mem;
use crate::errors::JobError;
use std::thread;

pub struct Job {
    pub commands: Vec<Call>,
    pub compile_errors: Vec<JobError>,
    pub runtime_errors: Vec<JobError>,
}

impl Job {
    pub fn new() -> Job {
        Job {
            commands: Vec::new(),
            compile_errors: Vec::new(),
            runtime_errors: Vec::new(),
        }
    }

    pub fn to_string(&self) -> String {
        let el: Vec<String> = self.commands.iter()
            .map(|c| String::from(c.get_name()))
            .collect();
        return el.join(" | ");
    }

    pub fn run(&mut self, state: &State) -> Result<(), ()> {
        if !self.commands.is_empty() && self.compile_errors.is_empty() {
            let (mut prev_output, mut prev_input) = streams();
            drop(prev_output);
            for c in &mut self.commands {
                let (mut output, mut input) = streams();

                let mut cc = c.clone();
                thread::spawn(move || {
                    match cc.run(&mut prev_input, &mut output) {
                        Ok(_) => {
                        }
                        Err(err) => {
//                            self.runtime_errors.push(err);
  //                          break;
                        }
                    }
                });
                prev_input = input;
            }
            match self.commands.last() {
                Some(command) => print(&mut prev_input, command.get_output_type()),
                None => {}
            }
        }
        return if self.runtime_errors.is_empty() { Ok(()) } else { Err(()) };
    }

    pub fn mutate(&mut self, state: &mut State) -> Result<(), ()> {
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
        return if self.runtime_errors.is_empty() { Ok(()) } else { Err(()) };
    }
}
