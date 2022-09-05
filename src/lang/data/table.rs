use crate::lang::errors::{argument_error_legacy, error, CrushError, CrushResult};
use crate::lang::pipe::CrushStream;
use crate::lang::value::ValueType;
use crate::lang::{data::r#struct::Struct, value::Value};
use crate::util::replace::Replace;
use chrono::Duration;
use std::fmt::{Display, Formatter};

#[derive(PartialEq, PartialOrd, Clone)]
pub struct Table {
    types: Vec<ColumnType>,
    rows: Vec<Row>,
}

impl Table {
    pub fn new(types: Vec<ColumnType>, rows: Vec<Row>) -> Table {
        Table { types, rows }
    }

    pub fn materialize(mut self) -> CrushResult<Table> {
        Ok(Table {
            types: ColumnType::materialize(&self.types)?,
            rows: self.rows.drain(..).map(|r| r.materialize()).collect::<CrushResult<Vec<_>>>()?,
        })
    }

    pub fn types(&self) -> &[ColumnType] {
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
            row_type: rows.types().to_vec(),
            rows,
        }
    }
}

impl CrushStream for TableReader {
    fn read(&mut self) -> Result<Row, CrushError> {
        if self.idx >= self.rows.rows().len() {
            return error("EOF");
        }
        self.idx += 1;
        Ok(self
            .rows
            .rows
            .replace(self.idx - 1, Row::new(vec![Value::Empty()])))
    }

    fn read_timeout(
        &mut self,
        _timeout: Duration,
    ) -> Result<Row, crate::lang::pipe::RecvTimeoutError> {
        match self.read() {
            Ok(r) => Ok(r),
            Err(_) => Err(crate::lang::pipe::RecvTimeoutError::Disconnected),
        }
    }

    fn types(&self) -> &[ColumnType] {
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

    pub fn into_struct(self, types: &[ColumnType]) -> Struct {
        Struct::from_vec(self.cells, types.to_vec())
    }

    pub fn push(&mut self, value: Value) {
        self.cells.push(value);
    }

    pub fn append(&mut self, values: &mut Vec<Value>) {
        self.cells.append(values);
    }

    pub fn materialize(mut self) -> CrushResult<Row> {
        Ok(Row {
            cells: self.cells.drain(..).map(|c| c.materialize()).collect::<CrushResult<Vec<_>>>()?,
        })
    }
}

impl From<Row> for Vec<Value> {
    fn from(row: Row) -> Vec<Value> {
        row.cells
    }
}


#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColumnType {
    pub name: String,
    pub cell_type: ValueType,
}

impl ColumnType {
    pub fn materialize(input: &[ColumnType]) -> CrushResult<Vec<ColumnType>> {
        let mut res = Vec::new();

        for col in input
            .iter() {
            res.push(ColumnType {
                name: col.name.clone(),
                cell_type: col.cell_type.materialize()?,
            });
        }
        Ok(res)
    }

    pub fn new(name: &str, cell_type: ValueType) -> ColumnType {
        ColumnType {
            name: name.to_string(),
            cell_type,
        }
    }
}

impl Display for ColumnType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.name.fmt(f)?;
        f.write_str("=(")?;
        self.cell_type.fmt(f)?;
        f.write_str(")")
    }
}

pub trait ColumnVec {
    fn find_str(&self, needle: &str) -> CrushResult<usize>;
    fn find(&self, needle: &[String]) -> CrushResult<usize>;
}

impl ColumnVec for &[ColumnType] {
    fn find_str(&self, needle: &str) -> CrushResult<usize> {
        for (idx, field) in self.iter().enumerate() {
            if field.name == needle {
                return Ok(idx);
            }
        }
        argument_error_legacy(
            format!(
                "Unknown column {}, available columns are {}",
                needle,
                self.iter()
                    .map(|t| t.name.to_string())
                    .collect::<Vec<String>>()
                    .join(", "),
            )
                .as_str(),
        )
    }

    fn find(&self, needle_vec: &[String]) -> CrushResult<usize> {
        if needle_vec.len() != 1 {
            argument_error_legacy("Expected direct field")
        } else {
            let needle = &needle_vec[0];
            for (idx, field) in self.iter().enumerate() {
                if &field.name == needle {
                    return Ok(idx);
                }
            }

            error(
                format!(
                    "Unknown column {}, available columns are {}",
                    needle,
                    self.iter()
                        .map(|t| t.name.to_string())
                        .collect::<Vec<String>>()
                        .join(", "),
                )
                    .as_str(),
            )
        }
    }
}
