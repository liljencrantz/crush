use std::collections::HashMap;
use crate::stream::{InputStream, OutputStream};
use crate::result::{CellType, Row, Cell, CellDataType};
use std::rc::Rc;

pub trait Call {
    fn get_name(&self) -> &String;
    fn get_arguments(&self) -> &Vec<String>;
    fn get_output(&self) -> &Vec<CellType>;
    fn run(&mut self, input: &mut dyn InputStream, output: &mut dyn OutputStream);
}

struct InternalCall {
    name: String,
    arguments: Vec<String>,
    output: Vec<CellType>,
    call: &'static dyn Fn(&mut dyn InputStream, &mut dyn OutputStream),
}

impl Call for InternalCall {

    fn get_name(&self) -> &String {
        return &self.name;
    }

    fn get_arguments(&self) -> &Vec<String> {
        return &self.arguments;
    }

    fn get_output(&self) -> &Vec<CellType> {
        return &self.output;
    }

    fn run(&mut self, input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        let c = &self.call;
        c(input, output);
    }
}

pub trait Command {
    fn call(&self, arguments: Vec<String>) -> Box<dyn Call>;
}

struct Ls {}

impl Ls {
    fn ls(input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        output.add(Row {
            cells: vec![Cell::STRING(String::from("foo")), Cell::INTEGER(123)]
        })
    }
}

impl Command for Ls {

    fn call(&self, arguments: Vec<String>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("ls"),
            arguments,
            output: vec![CellType {
                name: String::from("file"),
                cell_type: CellDataType::STRING,
            }],
            call: &Ls::ls,
        })
    }
}

struct Pwd {}

impl Pwd {
    fn pwd(input: &mut dyn InputStream, output: &mut dyn OutputStream) {
        output.add(Row {
            cells: vec![Cell::STRING(String::from(std::env::current_dir().expect("Oh no!").to_str().expect("Oh no")))]
        })
    }
}

impl Command for Pwd {

    fn call(&self, arguments: Vec<String>) -> Box<dyn Call> {
        return Box::new(InternalCall {
            name: String::from("pwd"),
            arguments,
            output: vec![CellType {
                name: String::from("directory"),
                cell_type: CellDataType::STRING,
            }],
            call: &Pwd::pwd,
        })
    }
}

pub struct CommandMap {
    commands: HashMap<String, Rc<dyn Command>>,
}

impl CommandMap {
    pub fn new() -> CommandMap {
        let mut commands: HashMap<String, Rc<dyn Command> > = HashMap::new();
        commands.insert(String::from("ls"), Rc::new(Ls {}));
        commands.insert(String::from("pwd"), Rc::new(Pwd {}));
        let res = CommandMap {
            commands,
        };
        return res;
    }

    pub fn call(&self, name: &String, arguments: Vec<String>) -> Box<dyn Call>{
        return self.commands.get(name).expect("Unknown command!").call(arguments);
    }
}
