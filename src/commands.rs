use std::collections::HashMap;
use crate::stream::{InputStream, OutputStream};
use crate::result::{CellType, Row, Cell, CellDataType, Argument};
use crate::state::State;
use crate::errors::CompileError;
extern crate map_in_place;

pub trait Call {
    fn get_name(&self) -> &String;
    fn get_arguments(&self) -> &Vec<Argument>;
    fn get_input_type(&self) -> &Vec<CellType>;
    fn get_output_type(&self) -> &Vec<CellType>;
    fn run(&mut self, input: &mut dyn InputStream, output: &mut dyn OutputStream);
    fn mutate(&mut self, state: &mut State);
}

struct InternalCall {
    name: String,
    input_type: Vec<CellType>,
    arguments: Vec<Argument>,
    output_type: Vec<CellType>,
    command: Box<dyn InternalCommand>,
}

impl Call for InternalCall {
    fn get_name(&self) -> &String {
        return &self.name;
    }

    fn get_arguments(&self) -> &Vec<Argument> {
        return &self.arguments;
    }

    fn get_input_type(&self) -> &Vec<CellType> {
        return &self.input_type;
    }

    fn get_output_type(&self) -> &Vec<CellType> {
        return &self.output_type;
    }

    fn run(&mut self, input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        self.command.run(&self.input_type, &self.arguments, input, output);
    }

    fn mutate(&mut self, state: &mut State) {
        self.command.mutate(&self.input_type, &self.arguments, state);
    }
}

pub trait Command {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Box<dyn Call>;
}

pub trait InternalCommand {
    fn run(&mut self, _input_type: &Vec<CellType>, _arguments: &Vec<Argument>, _input: &mut dyn InputStream, _output: &mut dyn OutputStream) {}
    fn mutate(&mut self, _input_type: &Vec<CellType>, _arguments: &Vec<Argument>, _state: &mut State) {}
}

#[derive(Clone)]
struct Ls {}

impl Command for Ls {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("ls"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: vec![CellType {
                name: String::from("file"),
                cell_type: CellDataType::String,
            }],
            command: Box::new(self.clone()),
        });
    }
}

impl InternalCommand for Ls {
    fn run(&mut self, _input_type: &Vec<CellType>, _arguments: &Vec<Argument>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        output.add(Row {
            cells: vec![Cell::String(String::from("foo")), Cell::Integer(123)]
        })
    }
}

#[derive(Clone)]
struct Pwd {}

impl InternalCommand for Pwd {
    fn run(&mut self, _input_type: &Vec<CellType>, _arguments: &Vec<Argument>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        output.add(Row {
            cells: vec![Cell::String(String::from(std::env::current_dir().expect("Oh no!").to_str().expect("Oh no")))]
        })
    }
}

impl Command for Pwd {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("pwd"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: vec![CellType {
                name: String::from("directory"),
                cell_type: CellDataType::String,
            }],
            command: Box::new(self.clone()),
        });
    }
}

#[derive(Clone)]
struct Echo {}

impl InternalCommand for Echo {
    fn run(&mut self, _input_type: &Vec<CellType>, arguments: &Vec<Argument>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        let g = arguments.iter().map(|c| c.cell.clone());
        output.add(Row {
            cells: g.collect()
        })
    }
}

impl Command for Echo {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("echo"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: vec![],
            command: Box::new(self.clone()),
        });
    }
}

#[derive(Clone)]
struct Cd {}

impl InternalCommand for Cd {
    fn mutate(&mut self, _input_type: &Vec<CellType>, arguments: &Vec<Argument>, _state: &mut State) {
        let dir = arguments.get(0).expect("AAA");
        match &dir.cell {
            Cell::String(val) => { std::env::set_current_dir(val); }
            _ => { println!("OH NOES!"); }
        }
    }
}

impl Command for Cd {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("cd"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: vec![],
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

    pub fn call(&self, name: &String, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, CompileError> {
        return match self.commands.get(name) {
            Some(cmd) => Result::Ok(cmd.call(input_type, arguments)),
            None => Result::Err(CompileError { message: String::from("Unknown command!")}),
        };
    }
}
