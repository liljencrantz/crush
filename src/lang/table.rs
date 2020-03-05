use crate::lang::{value::Value, r#struct::Struct};
use crate::lang::errors::{CrushError, error, CrushResult};
use crate::lang::stream::{Readable, InputStream};
use crate::util::replace::Replace;
use crate::lang::value::ValueType;

#[derive(Debug, PartialEq, PartialOrd, Clone)]
pub struct Table {
    types: Vec<ColumnType>,
    rows: Vec<Row>,
}

impl Table {
    pub fn new(types: Vec<ColumnType>, rows: Vec<Row>) -> Table {
        Table {types, rows}
    }

    pub fn materialize(mut self) -> Table {
        Table {
            types: ColumnType::materialize(&self.types),
            rows: self.rows.drain(..).map(|r| r.materialize()).collect(),
        }
    }

    pub fn types(&self) -> &Vec<ColumnType> {
        &self.types
    }

    pub fn rows(&self) -> &Vec<Row> {
        &self.rows
    }

    pub fn reader(self) -> TableReader {
        TableReader::new(self)
    }
}

pub struct TableReader {
    idx: usize,
    rows: Table,
    row_type: Vec<ColumnType>,
}

impl TableReader {
    pub fn new(rows: Table) -> TableReader {
        TableReader {
            idx: 0,
            row_type: rows.types().clone(),
            rows,
        }
    }
}

impl Readable for TableReader {

    fn read(&mut self) -> Result<Row, CrushError> {
        if self.idx >= self.rows.rows().len() {
            return error("EOF");
        }
        self.idx += 1;
        return Ok(self.rows.rows.replace(self.idx - 1, Row::new(vec![Value::Integer(0)])));
    }

    fn types(&self) -> &Vec<ColumnType> {
        &self.row_type
    }
}

#[derive(PartialEq, PartialOrd, Debug, Eq, Hash, Clone)]
pub struct Row {
    cells: Vec<Value>,
}

impl Row {
    pub fn new(cells: Vec<Value>) -> Row {
        Row { cells }
    }

    pub fn cells(&self) -> &Vec<Value> {
        &self.cells
    }

    pub fn into_struct(self, types: &Vec<ColumnType>) -> Struct {
        Struct::from_vec(self.cells, types.clone())
    }

    pub fn into_vec(self) -> Vec<Value> {
        self.cells
    }

    pub fn push(&mut self, value: Value) {
        self.cells.push(value);
    }

    pub fn append(&mut self, values: &mut Vec<Value>) {
        self.cells.append(values);
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn materialize(mut self) -> Row {
        Row {
            cells: self.cells.drain(..).map(|c| c.materialize()).collect(),
        }
    }

    pub fn replace(&mut self, idx: usize, value: Value) -> Value {
        self.cells.replace(idx, value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnType {
    pub name: Option<Box<str>>,
    pub cell_type: ValueType,
}

impl ColumnType {

    pub fn materialize(input: &Vec<ColumnType>) -> Vec<ColumnType> {
        input
            .iter()
            .map(|col| ColumnType {name: col.name.clone(), cell_type: col.cell_type.materialize()})
            .collect()
    }

    pub fn to_string(&self) -> String {
        match &self.name {
            None => self.cell_type.to_string(),
            Some(n) => format!("{}={}", n, self.cell_type.to_string()),
        }
    }

    pub fn new(name: Option<Box<str>>, cell_type: ValueType) -> ColumnType {
        ColumnType { name, cell_type }
    }

    pub fn named(name: &str, cell_type: ValueType) -> ColumnType {
        ColumnType {
            name: Some(Box::from(name)),
            cell_type,
        }
    }

    pub fn format_value(&self, v: &Value) -> String {
        match &self.name {
            Some(n) => format!("{}: {}", n, v.to_string()),
            None => v.to_string(),
        }
    }

    pub fn len_or_0(&self) -> usize {
        self.name.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    pub fn val_or_empty(&self) -> &str {
        self.name.as_ref().map(|v| v.as_ref()).unwrap_or("")
    }
}
