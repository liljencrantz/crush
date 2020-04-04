use crate::lang::{value::Value, r#struct::Struct};
use crate::lang::errors::{CrushError, error, CrushResult, argument_error};
use crate::lang::stream::{Readable};
use crate::util::replace::Replace;
use crate::lang::value::ValueType;

#[derive(PartialEq, PartialOrd, Clone)]
pub struct Table {
    types: Vec<ColumnType>,
    rows: Vec<Row>,
}

impl Table {
    pub fn new(types: Vec<ColumnType>, rows: Vec<Row>) -> Table {
        Table { types, rows }
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

#[derive(PartialEq, PartialOrd, Eq, Hash, Clone)]
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
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnType {
    pub name: Box<str>,
    pub cell_type: ValueType,
}

impl ColumnType {
    pub fn materialize(input: &Vec<ColumnType>) -> Vec<ColumnType> {
        input
            .iter()
            .map(|col| ColumnType { name: col.name.clone(), cell_type: col.cell_type.materialize() })
            .collect()
    }

    pub fn to_string(&self) -> String {
        format!("{}=({})", self.name, self.cell_type.to_string())
    }

    pub fn new(name: &str, cell_type: ValueType) -> ColumnType {
        ColumnType { name: Box::from(name), cell_type }
    }
}

pub trait ColumnVec {
    fn find_str(&self, needle: &str) -> CrushResult<usize>;
    fn find(&self, needle: &Vec<Box<str>>) -> CrushResult<usize>;
}

impl ColumnVec for Vec<ColumnType> {
    fn find_str(&self, needle: &str) -> CrushResult<usize> {
        for (idx, field) in self.iter().enumerate() {
            if field.name.as_ref() == needle {
                return Ok(idx);
            }
        }
        argument_error(format!(
            "Unknown column {}, available columns are {}",
            needle,
            self.iter().map(|t| t.name.to_string()).collect::<Vec<String>>().join(", "),
        ).as_str())
    }

    fn find(&self, needle_vec: &Vec<Box<str>>) -> CrushResult<usize> {
        if needle_vec.len() != 1 {
            argument_error("Expected direct field")
        } else {
            let needle = needle_vec[0].as_ref();
            for (idx, field) in self.iter().enumerate() {
                if field.name.as_ref() == needle {
                    return Ok(idx);
                }
            }

            error(format!(
                "Unknown column {}, available columns are {}",
                needle,
                self.iter().map(|t| t.name.to_string()).collect::<Vec<String>>().join(", "),
            ).as_str())
        }
    }
}
