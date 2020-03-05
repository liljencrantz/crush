use crate::lang::{value::ValueType, value::Value, table::ColumnType, table::Row};
use crate::lang::errors::{CrushResult, mandate, error};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use std::collections::HashMap;
use crate::lang::stream::Readable;
use crate::util::replace::Replace;

#[derive(Debug)]
#[derive(Clone)]
pub struct Dict {
    key_type: ValueType,
    value_type: ValueType,
    entries: Arc<Mutex<HashMap<Value, Value>>>,
}

impl Dict {
    pub fn new(key_type: ValueType, value_type: ValueType) -> Dict {
        if !key_type.is_hashable() {
            panic!("Tried to create dict with unhashable key type");
        }
        Dict {
            key_type,
            value_type,
            entries: Arc::new(Mutex::new(HashMap::new())) }
    }

    pub fn to_string(&self) -> String {
        let mut res = "dict{".to_string();
        let entries = self.entries.lock().unwrap();
        res += &entries.iter().map(|(k, v)| format!("{}: {}", k.to_string(), v.to_string())).collect::<Vec<String>>().join(" ");
        res += "}";
        res
    }

    pub fn len(&self) -> usize {
        let entries = self.entries.lock().unwrap();
        entries.len()
    }


    pub fn get(&self, key: &Value) -> Option<Value> {
        let entries = self.entries.lock().unwrap();
        entries.get(key).map(|c| c.clone())
    }

    pub fn remove(&self, key: &Value) -> Option<Value> {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(key)
    }

    pub fn insert(&self, key: Value, value: Value) {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(key, value);
    }

    pub fn key_type(&self) -> ValueType {
        self.key_type.clone()
    }
    pub fn value_type(&self) -> ValueType {
        self.value_type.clone()
    }
    pub fn dict_type(&self) -> ValueType {
        ValueType::Dict(Box::from(self.key_type.clone()), Box::from(self.value_type.clone()))
    }

    pub fn elements(&self) ->  Vec<(Value, Value)> {
        let mut entries = self.entries.lock().unwrap();
        entries.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }

    pub fn materialize(mut self) ->  Dict {
        let mut entries = self.entries.lock().unwrap();
        let map = entries.drain().map(|(k, v)| (k.materialize(), v.materialize())).collect();
        Dict {
            key_type: self.key_type.materialize(),
            value_type: self.value_type.materialize(),
            entries: Arc::new(Mutex::new(map))
        }
    }
}

impl std::hash::Hash for Dict {
    fn hash<H: Hasher>(&self, state: &mut H) {
    }
}

impl std::cmp::PartialEq for Dict {
    fn eq(&self, other: &Dict) -> bool {
        false
    }
}

impl std::cmp::PartialOrd for Dict {
    fn partial_cmp(&self, other: &Dict) -> Option<Ordering> {
        None
    }
}

pub struct DictReader {
    list: Vec<(Value, Value)>,
    idx: usize,
    types: Vec<ColumnType>,
}

impl DictReader {
    pub fn new(dict: Dict,
    ) -> DictReader {
        DictReader {
            types: vec![
                ColumnType::named("key", dict.key_type.clone()),
                ColumnType::named("value", dict.value_type.clone())],
            list: dict.elements(),
            idx: 0usize,
        }
    }
}

impl Readable for DictReader {
    fn read(&mut self) -> CrushResult<Row> {
        if self.idx >= self.list.len() {
            return error("End of stream");
        }
        let (a, b) = self.list.replace(self.idx, (Value::Bool(false), Value::Bool(false)));
        self.idx += 1;
        Ok(Row::new(vec![a, b]))
    }

    fn types(&self) -> &Vec<ColumnType> {
        &self.types
    }
}
