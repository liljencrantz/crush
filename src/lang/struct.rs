use crate::lang::table::ColumnType;
use crate::lang::value::Value;
use crate::lang::table::Row;
use std::sync::{Mutex, Arc};
use crate::lang::command::CrushCommand;

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::cmp::Ordering;
use crate::util::replace::Replace;

lazy_static! {
    pub static ref ROOT: Struct = {
        Struct::root()
    };
}

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
        unimplemented!()
    }
}

impl PartialOrd for Struct {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        unimplemented!()
    }
}

impl Struct {
    fn root() -> Struct {
        Struct::create(vec![
            (Box::from("__setattr__"), Value::Command(CrushCommand::command_undocumented(crate::lib::types::setattr, false))),
            (Box::from("__call_type__"), Value::Command(CrushCommand::command_undocumented(crate::lib::types::struct_call_type, false))),
        ], None)
    }

    pub fn new(vec: Vec<(Box<str>, Value)>) -> Struct {
        Struct::create(vec, Some(ROOT.clone()))
    }

    fn create(mut vec: Vec<(Box<str>, Value)>, parent: Option<Struct>) -> Struct {
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
                parent: Some(ROOT.clone()),
                lookup,
                cells,
            }))
        }
    }

    pub fn types(&self) -> Vec<ColumnType> {
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

    pub fn set(self, name: &str, value: Value) -> Option<Value> {
        let mut data = self.data.lock().unwrap();
        match data.lookup.get(name).cloned() {
            None => None,
            Some(idx) => Some(data.cells.replace(idx, value)),
        }
    }

    pub fn idx(&self, idx: usize) -> Option<Value> {
        let data = self.data.lock().unwrap();
        data.cells.get(idx).map(|v| v.clone())
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
}

impl ToString for Struct {
    fn to_string(&self) -> String {
        let t = self.types();
        let data = self.data.lock().unwrap();
        format!("{{{}}}",
                data.cells
                    .iter()
                    .zip(t)
                    .map(|(c, t)| t.format_value(c))
                    .collect::<Vec<String>>()
                    .join(", "))
    }
}
