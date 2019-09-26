use crate::stream::Stream;

pub enum CellDataType {
    STRING, INTEGER
}

pub struct CellType {
    name: String,
    cell_type: CellDataType,
}

pub enum Cell {
    STRING(String),
    INTEGER(i128),
}

pub struct Row {
    cells: Vec<Cell>,
}

pub struct Result {
    cell_types: Vec<CellType>,
    rows: crate::stream::Stream,
}

impl Result {
    pub fn new() -> Result {
        Result {
            cell_types: Vec::new(),
            rows: crate::stream::Stream {},
        }
    }

    pub fn to_string(&self) -> String {
        return String::from("weeee");
        //self.rows.iter().map()
    }
}
