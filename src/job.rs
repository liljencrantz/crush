use crate::state::State;
use crate::commands::Call;
use crate::stream::SerialStream;
use std::mem;
use crate::result::{CellType, Argument};
use crate::errors::JobError;

pub struct Job {
    pub src: String,
    pub commands: Vec<Box<dyn Call>>,
    pub compile_errors: Vec<JobError>,
    pub runtime_errors: Vec<JobError>,
}

impl Job {
    pub fn new(src: &String) -> Job {
        Job {
            src: String::from(src),
            commands: Vec::new(),
            compile_errors: Vec::new(),
            runtime_errors: Vec::new(),
        }
    }

    pub fn to_string(&self) -> String {
        let el: Vec<String> = self.commands.iter().map(|c| String::from(c.get_name())).collect();
        return el.join(" | ");
    }

    pub fn compile(&mut self, state: &State) -> Result<(), ()> {
        let calls: Vec<&str> = self.src.split('|').collect();
        let first_input: Vec<CellType> = Vec::new();
        let mut input = &first_input;
        'parse: for c in calls {
            let trimmed = c.trim();
            let pieces: Vec<&str> = trimmed.split(|c: char| c.is_ascii_whitespace()).collect();
            match pieces.split_first() {
                Some(wee) => {
                    let cmd = wee.0;
                    let arguments: Vec<Argument> = wee.1.iter().map(|s| Argument::from(*s)).collect();
                    let call = state.commands.call(&String::from(*cmd), input, &arguments);
                    match call {
                        Ok(c) => {
                            self.commands.push(c);
                            input = self.commands.last().expect("impossible").get_output_type();
                        }
                        Err(e) => {
                            self.compile_errors.push(e);
                            continue 'parse;
                        }
                    }
                }
                None => {
                    self.compile_errors.push(JobError { message: format!("Bad command {}", trimmed) });
                    continue 'parse;
                }
            }
        }
        return if self.compile_errors.is_empty() { Ok(()) } else { Err(()) };
    }

    pub fn run(&mut self, state: &State) -> Result<(), ()> {
        let mut input = SerialStream::new(Vec::new());
        let mut output = SerialStream::new(Vec::new());
        if !self.commands.is_empty() && self.compile_errors.is_empty() {
            for c in &mut self.commands {
                match c.run(&mut input, &mut output) {
                    Ok(_) => {
                        input.reset();
                        mem::swap(&mut input, &mut output)
                    }
                    Err(err) => {
                        self.runtime_errors.push(err);
                        break;
                    }
                }
            }
            input.print(self.commands.last().expect("Impossible").get_output_type());
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
