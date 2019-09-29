use crate::state::State;
use crate::commands::Call;
use crate::stream::SerialStream;
use std::mem;
use crate::result::{CellType, Argument};
use crate::errors::{CompileError, RuntimeError};

pub struct Job {
    pub src: String,
    pub commands: Vec<Box<dyn Call>>,
    pub compile_errors: Vec<CompileError>,
    pub runtime_errors: Vec<RuntimeError>,
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

    pub fn compile(&mut self, state: &State) -> Result<bool, bool>{
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
                    self.compile_errors.push(CompileError{message: format!("Bad command {}", trimmed)});
                    continue 'parse;
                }
            }
        }
        return if self.compile_errors.is_empty() {Ok(true)} else {Err(true)};
    }

    pub fn run(&mut self, state: &State) {
        let mut input = SerialStream::new(Vec::new());
        let mut output = SerialStream::new(Vec::new());
        if !self.commands.is_empty() {
            for c in &mut self.commands {
                c.run(&mut input, &mut output);
                input.reset();
                mem::swap(&mut input, &mut output)
            }
            input.print(self.commands.last().expect("Impossible").get_output_type());
        }
    }

    pub fn mutate(&mut self, state: &mut State) {
        if !self.commands.is_empty() {
            for c in &mut self.commands {
                c.mutate(state);
            }
        }
    }
}
