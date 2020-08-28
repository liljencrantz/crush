use crate::lang::errors::{error, CrushResult};
use crate::lang::serialization::model;
use crate::lang::serialization::model::{element, Element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState};
use crate::lang::table::{ColumnType, Row, Table};
use crate::lang::value::{Value, ValueType};

impl Serializable<ColumnType> for ColumnType {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<ColumnType> {
        if let element::Element::ColumnType(t) = elements[id].element.as_ref().unwrap() {
            Ok(ColumnType::new(
                &String::deserialize(t.name as usize, elements, state)?,
                ValueType::deserialize(t.r#type as usize, elements, state)?,
            ))
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
        let mut stype = model::ColumnType::default();
        stype.name = self.name.serialize(elements, state)? as u64;
        stype.r#type = self.cell_type.serialize(elements, state)? as u64;
        elements[idx].element = Some(element::Element::ColumnType(stype));
        Ok(idx)
    }
}

impl Serializable<Row> for Row {
    fn deserialize(
        id: usize,
        elements: &[Element],
        state: &mut DeserializationState,
    ) -> CrushResult<Row> {
        if let element::Element::Row(r) = elements[id].element.as_ref().unwrap() {
            let mut cells = Vec::new();
            for c in &r.cells {
                cells.push(Value::deserialize(*c as usize, elements, state)?);
            }
            Ok(Row::new(cells))
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
        let mut srow = model::Row::default();
        for r in self.cells() {
            srow.cells.push(r.serialize(elements, state)? as u64);
        }
        elements[idx].element = Some(element::Element::Row(srow));
        Ok(idx)
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
            Ok(Table::new(column_types, rows))
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
        for r in self.rows() {
            stable.rows.push(r.serialize(elements, state)? as u64);
        }
        elements[idx].element = Some(element::Element::Table(stable));
        Ok(idx)
    }
}
