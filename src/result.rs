#[derive(Clone)]
pub enum CellDataType {
    String,
    Integer,
}

#[derive(Clone)]
pub struct CellType {
    pub name: String,
    pub cell_type: CellDataType,
}

#[derive(Clone)]
pub enum Cell {
    String(String),
    Integer(i128),
//    Float(f64),
//    Row(Box<Row>),
//    Rows(Vec<Row>),
}

#[derive(Clone)]
pub struct Argument {
    pub name: String,
    pub cell: Cell,
}

trait IntoArgument {
    fn into_argument(self) -> Argument;
}

impl From<&String> for Argument {
    fn from(item: &String) -> Argument {
        return Argument {
            name:String::from(""),
            cell: Cell::String(item.clone()),
        }
    }
}

impl From<&str> for Argument {
    fn from(item: &str) -> Argument {
        return Argument::from(&String::from(item));
    }
}

pub struct Row {
    pub cells: Vec<Cell>,
}
