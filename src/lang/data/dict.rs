use crate::lang::errors::{CrushResult, command_error, error};
use crate::lang::pipe::CrushStream;
use crate::lang::{data::table::ColumnType, data::table::Row, value::Value, value::ValueType};
use crate::util::display_non_recursive::DisplayNonRecursive;
use crate::util::identity_arc::Identity;
use crate::util::replace::Replace;
use chrono::Duration;
use ordered_map::OrderedMap;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct Dict {
    key_type: ValueType,
    value_type: ValueType,
    entries: Arc<Mutex<OrderedMap<Value, Value>>>,
}

impl Identity for Dict {
    fn id(&self) -> u64 {
        self.entries.id()
    }
}

impl Dict {
    pub fn new(key_type: ValueType, value_type: ValueType) -> CrushResult<Dict> {
        if !key_type.is_hashable() {
            error("Tried to create dict with unhashable key type")
        } else {
            Ok(Dict {
                key_type,
                value_type,
                entries: Arc::new(Mutex::new(OrderedMap::new())),
            })
        }
    }

    pub fn new_with_data(
        key_type: ValueType,
        value_type: ValueType,
        entries: OrderedMap<Value, Value>,
    ) -> CrushResult<Dict> {
        if !key_type.is_hashable() {
            error("Tried to create dict with unhashable key type")
        } else {
            Ok(Dict {
                key_type,
                value_type,
                entries: Arc::new(Mutex::new(entries)),
            })
        }
    }

    pub fn len(&self) -> usize {
        let entries = self.entries.lock().unwrap();
        entries.len()
    }

    pub fn clear(&self) {
        let mut entries = self.entries.lock().unwrap();
        entries.clear()
    }

    pub fn copy(&self) -> Dict {
        let entries = self.entries.lock().unwrap();
        Dict {
            key_type: self.key_type.clone(),
            value_type: self.value_type.clone(),
            entries: Arc::new(Mutex::new(entries.clone())),
        }
    }

    pub fn get(&self, key: &Value) -> Option<Value> {
        let entries = self.entries.lock().unwrap();
        entries.get(key).map(|c| c.clone())
    }

    pub fn contains(&self, key: &Value) -> bool {
        let entries = self.entries.lock().unwrap();
        entries.contains_key(key)
    }

    pub fn remove(&self, key: &Value) -> Option<Value> {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(key)
    }

    pub fn insert(&self, key: Value, value: Value) -> CrushResult<()> {
        let mut entries = self.entries.lock().unwrap();
        if !self.key_type.is(&key) {
            return command_error(format!(
                "Invalid key type, expected {}, got {}.",
                self.key_type.to_string(),
                key.value_type().to_string()
            ));
        }
        if !self.value_type.is(&value) {
            return command_error(format!(
                "Invalid value type, expected {}, got {}.",
                self.value_type.to_string(),
                value.value_type().to_string()
            ));
        }
        entries.insert(key, value);
        Ok(())
    }

    pub fn key_type(&self) -> ValueType {
        self.key_type.clone()
    }

    pub fn value_type(&self) -> ValueType {
        self.value_type.clone()
    }
    pub fn dict_type(&self) -> ValueType {
        ValueType::Dict(
            Box::from(self.key_type.clone()),
            Box::from(self.value_type.clone()),
        )
    }

    pub fn elements(&self) -> Vec<(Value, Value)> {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn materialize(self) -> CrushResult<Dict> {
        let mut entries = self.entries.lock().unwrap();
        let mut map = OrderedMap::with_capacity(entries.len());
        for (k, v) in entries.drain() {
            map.insert(k.materialize()?, v.materialize()?);
        }
        Ok(Dict {
            key_type: self.key_type.materialize()?,
            value_type: self.value_type.materialize()?,
            entries: Arc::new(Mutex::new(map)),
        })
    }
}

impl PartialEq for Dict {
    fn eq(&self, other: &Dict) -> bool {
        if self.id() == other.id() {
            return true;
        }
        let us = self.entries.lock().unwrap().clone();
        let them = other.entries.lock().unwrap().clone();
        if us.len() != them.len() {
            return false;
        }
        for (k, v) in us.iter() {
            let them_value = them.get(k);
            match them_value {
                None => return false,
                Some(v2) => {
                    if !v.eq(v2) {
                        return false;
                    }
                }
            }
        }
        true
    }
}

impl DisplayNonRecursive for Dict {
    fn fmt_non_recursive(
        &self,
        f: &mut Formatter<'_>,
        seen: &mut HashSet<u64>,
    ) -> std::fmt::Result {
        if seen.contains(&self.id()) {
            return f.write_str("...");
        }
        seen.insert(self.id());

        f.write_str("dict{")?;
        let entries = self.entries.lock().unwrap();
        let mut first = true;
        for (k, v) in &mut entries.iter() {
            if first {
                first = false;
            } else {
                f.write_str(" ")?;
            }
            k.fmt_non_recursive(f, seen)?;
            f.write_str(": ")?;
            v.fmt_non_recursive(f, seen)?;
        }
        f.write_str("}")?;
        Ok(())
    }
}

impl Display for Dict {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let mut seen = HashSet::new();
        self.fmt_non_recursive(f, &mut seen)
    }
}

impl PartialOrd for Dict {
    fn partial_cmp(&self, _other: &Dict) -> Option<Ordering> {
        None
    }
}

pub struct DictReader {
    list: Vec<(Value, Value)>,
    idx: usize,
    types: Vec<ColumnType>,
}

impl DictReader {
    pub fn new(dict: Dict) -> DictReader {
        DictReader {
            types: vec![
                ColumnType::new("key", dict.key_type.clone()),
                ColumnType::new("value", dict.value_type.clone()),
            ],
            list: dict.elements(),
            idx: 0usize,
        }
    }
}

impl CrushStream for DictReader {
    fn read(&mut self) -> CrushResult<Row> {
        if self.idx >= self.list.len() {
            return error("End of stream");
        }
        let (a, b) = self
            .list
            .replace(self.idx, (Value::Bool(false), Value::Bool(false)));
        self.idx += 1;
        Ok(Row::new(vec![a, b]))
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
        &self.types
    }
}

impl Into<Value> for Dict {
    fn into(self) -> Value {
        Value::Dict(self)
    }
}
