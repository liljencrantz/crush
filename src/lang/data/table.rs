/**
Code related to Table, TableInputStream and
 */
use crate::lang::errors::{argument_error_legacy, CrushError, CrushResult, error};
use crate::lang::pipe::CrushStream;
use crate::lang::value::ValueType;
use crate::lang::{data::r#struct::Struct, value::Value};
use chrono::Duration;
use std::fmt::{Display, Formatter};
use std::sync::Arc;
use crate::lang::serialization::{DeserializationState, model, Serializable, SerializationState};
use crate::lang::serialization::model::{element, Element};

#[derive(PartialEq, PartialOrd, Clone)]
pub struct Table {
    types: Vec<ColumnType>,
    rows: Arc<[Row]>,
    materialized: bool,
}

pub struct Iter {
    table: Table,
    idx: usize,
}

impl Iterator for Iter {
    type Item = Row;

    fn next(&mut self) -> Option<Self::Item> {
        self.idx += 1;
        if let Ok(v) = self.table.row(self.idx - 1) {
            Some(v)
        } else {
            None
        }
    }
}

impl From<(Vec<ColumnType>, Vec<Row>)> for Table {
    fn from((types, rows): (Vec<ColumnType>, Vec<Row>)) -> Self {
        Table { types, rows: Arc::from(rows), materialized: false }
    }
}

impl Table {
    pub fn materialize(self) -> CrushResult<Table> {
        if self.materialized {
            Ok(self.clone())
        } else {
            let rows: Vec<Row> = self.rows.to_vec()
                .drain(..)
                .map(|r| r.materialize())
                .collect::<CrushResult<Vec<_>>>()?;
            Ok(Table {
                types: ColumnType::materialize(&self.types)?,
                materialized: true,
                rows: Arc::from(rows),
            })
        }
    }

    pub fn iter(&self) -> Iter {
        Iter {
            table: self.clone(),
            idx: 0
        }
    }

    pub fn types(&self) -> &[ColumnType] {
        &self.types
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn row(&self, idx: usize) -> CrushResult<Row> {
        if idx >= self.rows.len() {
            error("Index out of bounds")
        } else {
            Ok(self.rows[idx].clone())
        }
    }
}

pub struct TableReader {
    idx: usize,
    rows: Table,
}

impl TableReader {
    pub fn new(rows: Table) -> TableReader {
        TableReader {
            idx: 0,
            rows,
        }
    }
}

impl CrushStream for TableReader {
    fn read(&mut self) -> Result<Row, CrushError> {
        if self.idx >= self.rows.len() {
            return error("EOF");
        }
        self.idx += 1;
        Ok(self
            .rows
            .rows[self.idx - 1]
            .clone())
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
        self.rows.types()
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

    pub fn into_cells(self) -> Vec<Value> {
        self.cells
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
        f.write_str("=($")?;
        self.cell_type.fmt(f)?;
        f.write_str(")")
    }
}

pub trait ColumnVec {
    fn find(&self, needle: &str) -> CrushResult<usize>;
}

impl ColumnVec for &[ColumnType] {
    fn find(&self, needle: &str) -> CrushResult<usize> {
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
}

impl Serializable<Table> for Table {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Table> {
        if let element::Element::Table(lt) = elements[id].element.as_ref().unwrap() {
            let mut column_types = Vec::new();
            let mut rows = Vec::new();
            for ct in &lt.column_types {
                column_types.push(ColumnType::deserialize(*ct as usize, elements, state)?);
            }
            for r in &lt.rows {
                rows.push(Row::deserialize(*r as usize, elements, state)?);
            }
            Ok(Table::from((column_types, rows)))
        } else {
            error("Expected a table")
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(model::Element::default());
        let mut stable = model::Table::default();
        for t in self.types() {
            stable
                .column_types
                .push(t.serialize(elements, state)? as u64);
        }
        for r in self.rows.iter() {
            stable.rows.push(r.serialize(elements, state)? as u64);
        }
        elements[idx].element = Some(element::Element::Table(stable));
        Ok(idx)
    }
}
