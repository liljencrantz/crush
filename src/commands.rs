use std::collections::HashMap;
use crate::stream::{InputStream, OutputStream};
use crate::result::{CellType, Row, Cell, CellDataType, Argument};
use crate::state::State;
use crate::errors::CompileError;
use std::{fs, io};

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

impl Ls {
    fn run_internal(
        &mut self,
        _input_type: &Vec<CellType>,
        _arguments: &Vec<Argument>,
        _input: &mut dyn InputStream, output: &mut dyn OutputStream) -> Result<bool, io::Error> {
        let dirs = fs::read_dir(".");
        for maybe_entry in dirs? {
            let entry = maybe_entry?;
            match entry.file_name().into_string() {
                Ok(name) =>
                    output.add(Row {
                        cells: vec![Cell::Text(name)]
                    }),
                _ => {}
            }
        }
        Ok(true)
    }
}

impl Command for Ls {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("ls"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: vec![CellType {
                name: String::from("file"),
                cell_type: CellDataType::Text,
            }],
            command: Box::new(self.clone()),
        });
    }
}

impl InternalCommand for Ls {
    fn run(&mut self, _input_type: &Vec<CellType>, _arguments: &Vec<Argument>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        self.run_internal(_input_type, _arguments, _input, output);
    }
}

#[derive(Clone)]
struct Pwd {}

impl InternalCommand for Pwd {
    fn run(&mut self, _input_type: &Vec<CellType>, _arguments: &Vec<Argument>, _input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        output.add(Row {
            cells: vec![Cell::Text(String::from(std::env::current_dir().expect("Oh no!").to_str().expect("Oh no")))]
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
                cell_type: CellDataType::Text,
            }],
            command: Box::new(self.clone()),
        });
    }
}

#[derive(Clone)]
struct Filter {}

impl InternalCommand for Filter {
    fn run(&mut self, _input_type: &Vec<CellType>, _arguments: &Vec<Argument>, input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        loop {
            match input.next() {
                Some(row) => {
                    if row.cells[0] == _arguments[0].cell {
                        output.add(row);
                    }
                },
                None => {
                    break;
                },
            }
        }
    }
}

impl Command for Filter {
    fn call(&self, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("filter"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type: input_type.clone(),
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
        let output_type = arguments
            .iter()
            .map(|a| CellType { name: a.name.clone(), cell_type: a.cell.cell_data_type() })
            .collect();
        return Box::new(InternalCall {
            name: String::from("echo"),
            input_type: input_type.clone(),
            arguments: arguments.clone(),
            output_type,
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
            Cell::Text(val) => { std::env::set_current_dir(val); }
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
        commands.insert(String::from("filter"), Box::new(Filter {}));
        let res = Namespace {
            commands,
        };
        return res;
    }

    pub fn call(&self, name: &String, input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Box<dyn Call>, CompileError> {
        return match self.commands.get(name) {
            Some(cmd) => Result::Ok(cmd.call(input_type, arguments)),
            None => Result::Err(CompileError { message: String::from("Unknown command!") }),
        };
    }
}
