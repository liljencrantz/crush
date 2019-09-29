use crate::state::State;
use crate::commands::{Call};
use crate::stream::SerialStream;
use std::mem;
use crate::result::{ CellType, Argument };
use crate::errors::CompileError;

pub struct Job {
    src: String,
    commands: Vec<Box<dyn Call>>,
    errors: Vec<CompileError>,
}

impl Job {
    pub fn new(src: &String) -> Job {
        Job {
            src: String::from(src),
            commands: Vec::new(),
            errors: Vec::new(),
        }
    }

    pub fn to_string(&self) -> String {
        let el: Vec<String> = self.commands.iter().map(|c| String::from(c.get_name())).collect();
        return el.join(" | ");
    }

    pub fn compile(&mut self, state: &State) {
        let calls: Vec<&str> = self.src.split('|').collect();
        let first_input: Vec<CellType> = Vec::new();
        let mut input= &first_input;
        'parse: for c in calls {
            let trimmed = c.trim();
            let pieces: Vec<&str> = trimmed.split(|c: char| c.is_ascii_whitespace()).collect();
            let wee = pieces.split_first().expect("Oh noes!!!");
            let cmd = wee.0;
            let arguments: Vec<Argument> = wee.1.iter().map(|s| Argument::from(*s)).collect();
            let call = state.commands.call(&String::from(*cmd), input, &arguments);
            match call {
                Ok(c) => {
                    self.commands.push(c);
                    input = self.commands.last().expect("impossible").get_output_type();
                }
                Err(e) => {
                    self.errors.push(e);
                    break 'parse;
                }
            }
        }
    }

    pub fn run(&mut self, state: &mut State) {
        let mut input = SerialStream::new(Vec::new());
        let mut output = SerialStream::new(Vec::new());

        for c in &mut self.commands {
            c.run(&mut input, &mut output);
            input.reset();
            mem::swap(&mut input, &mut output)
        }
        for c in &mut self.commands {
            c.mutate(state);
        }
        input.print();
    }
}
