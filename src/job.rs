use crate::state::State;
use crate::result::Result;
use crate::commands::Command;
use std::rc::Rc;

pub struct Call {
    pub command: Rc<dyn Command>,
    pub arguments: Vec<String>,
}

pub struct Job {
    src: String,
    commands: Vec<Call>,
}

impl Job {
    pub fn new(src: &String) -> Job {
        Job {
            src: String::from(src),
            commands: Vec::new(),
        }
    }

    pub fn to_string(&self) -> String {
        let el: Vec<String> = self.commands.iter().map(|c| String::from(&c.command.get_name())).collect();
        return el.join(" | ");
    }

    pub fn compile(&mut self, state: &State) {
        let el: Vec<&str> = self.src.split('|').collect();
        for c in el {
            self.commands.push(Call {
                command: state.commands.get(&String::from("ls")),
                arguments: Vec::new(),
            })
        }
    }

    pub fn run(&mut self, state: &mut State, result: &mut Result) {}
}
