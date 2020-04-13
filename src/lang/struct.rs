use crate::lang::table::ColumnType;
use crate::lang::value::Value;
use crate::lang::table::Row;
use std::sync::{Mutex, Arc};
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;
use crate::util::replace::Replace;
use crate::util::identity_arc::Identity;

#[derive(Clone)]
struct StructData {
    parent: Option<Struct>,
    lookup: HashMap<Box<str>, usize>,
    cells: Vec<Value>,
}

#[derive(Clone)]
pub struct Struct {
    data: Arc<Mutex<StructData>>,
}

impl Identity for Struct {
    fn id(&self) -> u64 {
        self.data.id()
    }
}

impl Hash for Struct {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let data = self.data.lock().unwrap();
        data.cells.iter().for_each(|value| {
            value.hash(state);
        });
        let p = data.parent.clone();
        drop(data);
        p.hash(state);
    }
}

impl PartialEq for Struct {
    fn eq(&self, other: &Self) -> bool {
        let us = self.data.lock().unwrap().clone();
        let them = other.data.lock().unwrap().clone();
        if us.cells.len() != them.cells.len() {
            return false;
        }
        for (v1, v2) in us.cells.iter().zip(them.cells.iter()) {
            if !v1.eq(v2) {
                return false;
            }
        }
        for (name, idx) in us.lookup.iter() {
            match them.lookup.get(name) {
                None => return false,
                Some(idx2) => if !idx.eq(idx2) {
                    return false;
                },
            }
        }
        true
    }
}

impl PartialOrd for Struct {
    fn partial_cmp(&self, _other: &Self) -> Option<Ordering> {
        None
    }
}

impl Struct {
    pub fn new(mut vec: Vec<(Box<str>, Value)>, parent: Option<Struct>) -> Struct {
        let mut lookup = HashMap::new();
        let mut cells = Vec::new();
        vec.drain(..)
            .for_each(|(key, value)| {
                lookup.insert(key, cells.len());
                cells.push(value);
            });
        Struct {
            data: Arc::new(Mutex::new(StructData {
                parent,
                cells,
                lookup,
            }))
        }
    }

    pub fn from_vec(mut values: Vec<Value>, types: Vec<ColumnType>) -> Struct {
        let mut lookup = HashMap::new();
        let mut cells = Vec::new();

        values.drain(..)
            .zip(types)
            .for_each(|(value, column)| {
                lookup.insert(column.name, cells.len());
                cells.push(value);
            });
        Struct {
            data: Arc::new(Mutex::new(StructData {
                parent: None,
                lookup,
                cells,
            }))
        }
    }

    pub fn local_signature(&self) -> Vec<ColumnType> {
        let mut res = Vec::new();
        let data = self.data.lock().unwrap();
        let mut reverse_lookup = HashMap::new();
        for (key, value) in &data.lookup {
            reverse_lookup.insert(value.clone(), key);
        }
        for (idx, value) in data.cells.iter().enumerate() {
            res.push(ColumnType::new(reverse_lookup.get(&idx).unwrap(), value.value_type()));
        }
        res
    }

    pub fn local_elements(&self) -> Vec<(Box<str>, Value)> {
        let mut reverse_lookup = HashMap::new();
        let data = self.data.lock().unwrap();
        for (key, value) in &data.lookup {
            reverse_lookup.insert(value.clone(), key);
        }
        data.cells.iter().enumerate()
            .map(|(idx, v)| (reverse_lookup[&idx].to_string().into_boxed_str(), v.clone())).collect()
    }

    pub fn into_row(&self) -> Row {
        Row::new(self.data.lock().unwrap().cells.clone())
    }

    pub fn into_vec(&self) -> Vec<Value> {
        self.data.lock().unwrap().cells.clone()
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        let data = self.data.lock().unwrap();
        match data.lookup.get(name) {
            None => {
                let p = data.parent.clone();
                drop(data);
                match p {
                    None => None,
                    Some(parent) => parent.get(name),
                }
            }
            Some(idx) => Some(data.cells[*idx].clone()),
        }
    }

    pub fn keys(&self) -> Vec<Box<str>> {
        let mut fields = HashSet::new();
        self.fill_keys(&mut fields);
        fields.drain().collect()
    }

    fn fill_keys(&self, dest: &mut HashSet<Box<str>>) {
        let data = self.data.lock().unwrap();
        data.lookup.keys().for_each(|name| { dest.insert(name.clone()); });
        let parent = data.parent.clone();
        drop(data);
        if let Some(p) = parent {
            p.fill_keys(dest);
        }
    }

    pub fn set(&self, name: &str, value: Value) -> Option<Value> {
        let mut data = self.data.lock().unwrap();
        match data.lookup.get(name).cloned() {
            None => {
                let idx = data.lookup.len();
                data.lookup.insert(Box::from(name), idx);
                data.cells.push(value);
                None
            }
            Some(idx) => Some(data.cells.replace(idx, value)),
        }
    }

    pub fn materialize(&self) -> Struct {
        let data = self.data.lock().unwrap();
        Struct {
            data: Arc::new(Mutex::new(StructData {
                parent: data.parent.clone(),
                lookup: data.lookup.clone(),
                cells: data.cells.iter().map(|value| value.clone().materialize()).collect(),
            }))
        }
    }

    pub fn set_parent(&self, parent: Option<Struct>) {
        self.data.lock().unwrap().parent = parent;
    }
}

impl ToString for Struct {
    fn to_string(&self) -> String {
        let elements = self.local_elements();
        let data = self.data.lock().unwrap();
        let parent = data.parent.clone();
        drop(data);
        format!("data{} {}",
                parent.map(|p| format!(" parent=({})", p.to_string())).unwrap_or_else(|| "".to_string()),
                elements
                    .iter()
                    .map(|(c, t)| format!("{}=({})", c, t.to_string()))
                    .collect::<Vec<String>>()
                    .join(", "))
    }
}
