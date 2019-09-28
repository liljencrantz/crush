use crate::state::State;
use crate::commands::{Call};
use crate::stream::SerialStream;
use std::mem;
use crate::result::Cell;

pub struct Job {
    src: String,
    commands: Vec<Box<dyn Call>>,
}

impl Job {
    pub fn new(src: &String) -> Job {
        Job {
            src: String::from(src),
            commands: Vec::new(),
        }
    }

    pub fn to_string(&self) -> String {
        let el: Vec<String> = self.commands.iter().map(|c| String::from(c.get_name())).collect();
        return el.join(" | ");
    }

    pub fn compile(&mut self, state: &State) {
        let calls: Vec<&str> = self.src.split('|').collect();
        for c in calls {
            let trimmed = c.trim();
            let pieces: Vec<&str> = trimmed.split(|c: char| c.is_ascii_whitespace()).collect();
            let wee = pieces.split_first().expect("Oh noes!!!");
            let cmd = wee.0;
            let arguments: Vec<Cell> = wee.1.iter().map(|s:&&str| Cell::STRING(String::from(*s))).collect();
            //println!("cmd: {} args: {:?}", cmd, arguments);
            self.commands.push(state.commands.call(&String::from(*cmd), arguments));
        }
    }

    pub fn run(&mut self, state: &mut State) {
        let mut input = SerialStream::new(Vec::new());
        let mut output = SerialStream::new(Vec::new());

        for mut c in &mut self.commands {
            c.run(&mut input, &mut output);
            input.reset();
            mem::swap(&mut input, &mut output)
        }
        for mut c in &mut self.commands {
            c.mutate(state);
        }
        input.print();
    }
}
