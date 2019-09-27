use std::collections::HashMap;
use crate::stream::Stream;
use crate::result::CellType;
use std::rc::Rc;

pub trait Command {
    fn get_name(&self) -> String;
    fn get_output(&self, arguments: Vec<String>, input: Vec<CellType>) -> Vec<CellType>;
    fn run(&self, arguments: Vec<String>, input: Stream) -> Stream;
}

struct Ls {}

impl Command for Ls {

    fn get_name(&self) -> String {
        return String::from("ls")
    }

    fn get_output(&self, arguments: Vec<String>, input: Vec<CellType>) -> Vec<CellType> {
        return Vec::new();
    }

    fn run(&self, arguments: Vec<String>, input: Stream) -> Stream {
        return Stream::new(self.get_output(arguments, input.row_type));
    }
}

pub struct CommandMap {
    commands: HashMap<String, Rc<dyn Command>>,
}

impl CommandMap {
    pub fn new() -> CommandMap {
        let mut commands: HashMap<String, Rc<dyn Command> > = HashMap::new();
        commands.insert(String::from("ls"), Rc::new(Ls {}));
        let res = CommandMap {
            commands,
        };


        return res;
    }

    pub fn get(&self, name: &String) -> Rc<dyn Command>{
        return Rc::clone(self.commands.get(name).expect("Unknown command!"));
    }
}
