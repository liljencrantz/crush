use crate::data::{CellType, Cell};
use crate::errors::{JobError, mandate, JobResult};
use std::hash::Hasher;
use std::sync::{Arc, Mutex};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug)]
#[derive(Clone)]
pub struct Dict {
    key_type: CellType,
    value_type: CellType,
    entries: Arc<Mutex<HashMap<Cell, Cell>>>,
}

impl Dict {
    pub fn new(key_type: CellType, value_type: CellType) -> Dict {
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


    pub fn get(&self, key: &Cell) -> Option<Cell> {
        let entries = self.entries.lock().unwrap();
        entries.get(key).map(|c| c.partial_clone().unwrap())
    }

    pub fn remove(&self, key: &Cell) -> Option<Cell> {
        let mut entries = self.entries.lock().unwrap();
        entries.remove(key)
    }

    pub fn insert(&self, key: Cell, value: Cell) {
        let mut entries = self.entries.lock().unwrap();
        entries.insert(key, value);
    }

    pub fn key_type(&self) -> CellType {
        self.key_type.clone()
    }
    pub fn value_type(&self) -> CellType {
        self.value_type.clone()
    }
    pub fn dict_type(&self) -> CellType {
        CellType::Dict(Box::from(self.key_type.clone()), Box::from(self.value_type.clone()))
    }

    pub fn partial_clone(&self) -> Result<Dict, JobError> {
        Ok(self.clone())
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
