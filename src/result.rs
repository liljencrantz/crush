pub enum CellDataType {
    STRING, INTEGER
}

pub struct CellType {
    pub name: String,
    pub cell_type: CellDataType,
}

pub enum Cell {
    STRING(String),
    INTEGER(i128),
}

pub struct Row {
    pub cells: Vec<Cell>,
}
