use std::collections::HashMap;
use crate::stream::{InputStream, OutputStream};
use crate::result::{CellType, Row, Cell, CellDataType};
use crate::state::State;
extern crate map_in_place;
use map_in_place::MapVecInPlace;

pub trait Call {
    fn get_name(&self) -> &String;
    fn get_arguments(&self) -> &Vec<Cell>;
    fn get_output(&self) -> &Vec<CellType>;
    fn run(&mut self, input: &mut dyn InputStream, output: &mut dyn OutputStream);
    fn mutate(&mut self, state: &mut State);
}

struct InternalCall {
    name: String,
    arguments: Vec<Cell>,
    output: Vec<CellType>,
    command: Box<dyn InternalCommand>,
}

impl Call for InternalCall {
    fn get_name(&self) -> &String {
        return &self.name;
    }

    fn get_arguments(&self) -> &Vec<Cell> {
        return &self.arguments;
    }

    fn get_output(&self) -> &Vec<CellType> {
        return &self.output;
    }

    fn run(&mut self, input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        self.command.run(&self.arguments, input, output);
    }

    fn mutate(&mut self, state: &mut State) {
        self.command.mutate(&self.arguments, state);
    }
}

pub trait Command {
    fn call(&self, arguments: Vec<Cell>) -> Box<dyn Call>;
}

pub trait InternalCommand {
    fn run(&mut self, arguments: &Vec<Cell>, input: &mut dyn InputStream, output: &mut dyn OutputStream) {}
    fn mutate(&mut self, arguments: &Vec<Cell>, state: &mut State) {}
}

#[derive(Clone)]
struct Ls {}

impl Command for Ls {
    fn call(&self, arguments: Vec<Cell>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("ls"),
            arguments,
            output: vec![CellType {
                name: String::from("file"),
                cell_type: CellDataType::STRING,
            }],
            command: Box::new(self.clone()),
        });
    }
}

impl InternalCommand for Ls {
    fn run(&mut self, _arguments: &Vec<Cell>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        output.add(Row {
            cells: vec![Cell::STRING(String::from("foo")), Cell::INTEGER(123)]
        })
    }
}

#[derive(Clone)]
struct Pwd {}

impl InternalCommand for Pwd {
    fn run(&mut self, _arguments: &Vec<Cell>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        output.add(Row {
            cells: vec![Cell::STRING(String::from(std::env::current_dir().expect("Oh no!").to_str().expect("Oh no")))]
        })
    }
}

impl Command for Pwd {
    fn call(&self, arguments: Vec<Cell>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("pwd"),
            arguments,
            output: vec![CellType {
                name: String::from("directory"),
                cell_type: CellDataType::STRING,
            }],
            command: Box::new(self.clone()),
        });
    }
}

#[derive(Clone)]
struct Echo {}

impl InternalCommand for Echo {
    fn run(&mut self, arguments: &Vec<Cell>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        let mut cpy = arguments.clone();
        let g = cpy.map_in_place(|c| c.clone());
        output.add(Row {
            cells: g
        })
    }
}

impl Command for Echo {
    fn call(&self, arguments: Vec<Cell>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("echo"),
            arguments,
            output: vec![],
            command: Box::new(self.clone()),
        });
    }
}

#[derive(Clone)]
struct Cd {}

impl InternalCommand for Cd {
    fn mutate(&mut self, arguments: &Vec<Cell>, state: &mut State) {
        let dir = arguments.get(0).expect("AAA");
        match dir {
            Cell::STRING(val) => { std::env::set_current_dir(val); }
            _ => { println!("OH NOES!"); }
        }
    }
}

impl Command for Cd {
    fn call(&self, arguments: Vec<Cell>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("cd"),
            arguments,
            output: vec![],
            command: Box::new(self.clone()),
        });
    }
}

pub struct Namespace {
    commands: HashMap<String, Box<dyn Command>>,
}

impl Namespace {
    pub fn new() -> Namespace {
        let mut commands: HashMap<String, Box<dyn Command>> = HashMap::new();
        commands.insert(String::from("ls"), Box::new(Ls {}));
        commands.insert(String::from("pwd"), Box::new(Pwd {}));
        commands.insert(String::from("cd"), Box::new(Cd {}));
        commands.insert(String::from("echo"), Box::new(Echo {}));
        let res = Namespace {
            commands,
        };
        return res;
    }

    pub fn call(&self, name: &String, arguments: Vec<Cell>) -> Box<dyn Call> {
        let cmd = self.commands.get(name).expect("Unknown command!");
        return cmd.call(arguments);
    }
}
