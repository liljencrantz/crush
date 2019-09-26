
pub struct Command {
    pub name: String,
}

impl Command {
    fn get_output(arguments: Vec<String>, input: Vec<crate::result::CellType>) -> Vec<crate::result::CellType> {
        return Vec::new();
    }

    fn run(arguments: Vec<String>, input: PoshStream) -> PoshStream {
        return Vec::new();
    }
}

pub struct Call {
    pub name: String,
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
        let el: Vec<String> = self.commands.iter().map(|c| String::from(&c.name)).collect();
        return el.join(" | ");
    }

    pub fn compile(&mut self) {
        let el: Vec<&str> = self.src.split('|').collect();
        for c in el {
            self.commands.push(Call {
                name: String::from(c),
                arguments: Vec::new(),
            })
        }
    }

    pub fn run(&mut self, result: &mut crate::result::Result) {}
}
