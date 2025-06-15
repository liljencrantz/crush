use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;
use crate::lang::errors::{CrushError, CrushResult, error};
use crate::lang::help::Help;
use crate::lang::pipe::CrushStream;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::identity_arc::Identity;
use crate::util::replace::Replace;
use chrono::Duration;
use ordered_map::OrderedMap;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::fmt::{Display, Formatter};
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{Arc, Mutex};

static STRUCT_STREAM_TYPE: [ColumnType; 2] = [
    ColumnType::new("name", ValueType::String),
    ColumnType::new("value", ValueType::Any),
];

#[derive(Clone)]
struct StructData {
    parent: Option<Struct>,
    lookup: OrderedMap<String, usize>,
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
                Some(idx2) => {
                    if !idx.eq(idx2) {
                        return false;
                    }
                }
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
    pub fn empty(parent: Option<Struct>) -> Struct {
        let v: Vec<(String, Value)> = Vec::new();
        Struct::new(v, parent)
    }

    pub fn new(mut vec: Vec<(impl Into<String>, Value)>, parent: Option<Struct>) -> Struct {
        let mut lookup = OrderedMap::new();
        let mut cells = Vec::new();
        vec.drain(..).for_each(|(key, value)| {
            lookup.insert(key.into(), cells.len());
            cells.push(value);
        });
        Struct {
            data: Arc::new(Mutex::new(StructData {
                parent,
                cells,
                lookup,
            })),
        }
    }

    pub fn from_vec(mut values: Vec<Value>, types: Vec<ColumnType>) -> Struct {
        let mut lookup = OrderedMap::new();
        let mut cells = Vec::new();

        values.drain(..).zip(types).for_each(|(value, column)| {
            lookup.insert(column.name().to_string(), cells.len());
            cells.push(value);
        });
        Struct {
            data: Arc::new(Mutex::new(StructData {
                parent: None,
                lookup,
                cells,
            })),
        }
    }

    pub fn local_signature(&self) -> Vec<ColumnType> {
        let mut res = Vec::new();
        let data = self.data.lock().unwrap();
        let mut reverse_lookup = OrderedMap::new();
        for (key, value) in &data.lookup {
            reverse_lookup.insert(value.clone(), key);
        }
        for (idx, value) in data.cells.iter().enumerate() {
            res.push(ColumnType::new_from_string(
                reverse_lookup.get(&idx).unwrap().to_string(),
                value.value_type(),
            ));
        }
        res
    }

    pub fn local_elements(&self) -> Vec<(String, Value)> {
        let mut reverse_lookup = OrderedMap::new();
        let data = self.data.lock().unwrap();
        for (key, value) in &data.lookup {
            reverse_lookup.insert(value.clone(), key);
        }
        data.cells
            .iter()
            .enumerate()
            .map(|(idx, v)| (reverse_lookup[&idx].to_string(), v.clone()))
            .collect()
    }

    pub fn to_row(&self) -> Row {
        Row::new(self.data.lock().unwrap().cells.clone())
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

    pub fn keys(&self) -> Vec<String> {
        let mut fields = HashSet::new();
        self.fill_keys(&mut fields);
        fields.drain().collect()
    }

    fn fill_keys(&self, dest: &mut HashSet<String>) {
        let data = self.data.lock().unwrap();
        data.lookup.keys().for_each(|name| {
            dest.insert(name.clone());
        });
        let parent = data.parent.clone();
        drop(data);
        if let Some(p) = parent {
            p.fill_keys(dest);
        }
    }

    pub fn map(&self) -> OrderedMap<String, Value> {
        let mut map = OrderedMap::new();
        self.fill_map(&mut map);
        map
    }

    fn fill_map(&self, dest: &mut OrderedMap<String, Value>) {
        let data = self.data.lock().unwrap();
        data.lookup.iter().for_each(|(name, idx)| {
            if !dest.contains_key(name) {
                dest.insert(name.clone(), data.cells[*idx].clone());
            }
        });
        let parent = data.parent.clone();
        drop(data);
        if let Some(p) = parent {
            p.fill_map(dest);
        }
    }

    pub fn set(&self, name: &str, value: Value) -> Option<Value> {
        let mut data = self.data.lock().unwrap();
        match data.lookup.get(name).cloned() {
            None => {
                let idx = data.lookup.len();
                data.lookup.insert(name.to_string(), idx);
                data.cells.push(value);
                None
            }
            Some(idx) => Some(data.cells.replace(idx, value)),
        }
    }

    pub fn materialize(&self) -> CrushResult<Struct> {
        let data = self.data.lock().unwrap();
        Ok(Struct {
            data: Arc::new(Mutex::new(StructData {
                parent: data.parent.clone(),
                lookup: data.lookup.clone(),
                cells: data
                    .cells
                    .iter()
                    .map(|value| value.clone().materialize())
                    .collect::<CrushResult<Vec<_>>>()?,
            })),
        })
    }

    pub fn set_parent(&self, parent: Option<Struct>) {
        self.data.lock().unwrap().parent = parent;
    }
}

impl Display for Struct {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let elements = self.local_elements();
        let data = self.data.lock().unwrap();

        f.write_str("data")?;
        if let Some(parent) = data.parent.clone() {
            f.write_str(" parent=(")?;
            parent.fmt(f)?;
            f.write_str(")")?;
        }

        for (name, value) in elements.iter() {
            f.write_str(" ")?;
            name.fmt(f)?;
            f.write_str("=(")?;
            value.fmt(f)?;
            f.write_str(")")?;
        }
        Ok(())
    }
}

fn format_help(s: impl Into<String>, is_example: bool) -> String {
    if is_example {
        format!("    {}", s.into())
    } else {
        s.into()
    }
}

fn extract_help(value: Value, res: &mut Vec<String>, is_example: bool) {
    match value {
        Value::String(s) => res.push(format_help(s.deref(), is_example)),
        Value::List(l) => {
            for v in l.iter() {
                match v {
                    Value::String(s) => res.push(format_help(s.deref(), is_example)),
                    _ => res.push("<Invalid help item>".to_string()),
                }
            }
        }
        _ => res.push("<Invalid help item>".to_string()),
    }
}

impl Help for Struct {
    fn signature(&self) -> String {
        self.get("__signature__")
            .map(|v| v.to_string())
            .unwrap_or("type struct".to_string())
    }

    fn short_help(&self) -> String {
        self.get("__short_help__")
            .map(|v| v.to_string())
            .unwrap_or("A mapping from name to value".to_string())
    }

    fn long_help(&self) -> Option<String> {
        let mut res = Vec::new();

        if let Some(l) = self.get("__long_help__") {
            extract_help(l, &mut res, false)
        }

        let mut v = self.map().drain().collect::<Vec<_>>();
        if !v.is_empty() {
            if !res.is_empty() {
                res.push("".to_string());
            }

            res.push("This struct has the following fields:".to_string());

            v.sort_by(|a, b| a.0.cmp(&b.0));

            for el in v {
                res.push(format!("* `{}` {}", el.0, el.1.short_help()));
            }
        }

        if let Some(l) = self.get("__example__") {
            res.push("# Examples".to_string());
            extract_help(l, &mut res, true)
        }

        Some(res.join("\n"))
    }
}

pub struct StructReader {
    idx: usize,
    rows: Vec<(String, Value)>,
}

impl StructReader {
    pub fn new(s: Struct) -> StructReader {
        StructReader {
            idx: 0,
            rows: s.map().drain().collect(),
        }
    }
}

impl CrushStream for StructReader {
    fn read(&mut self) -> Result<Row, CrushError> {
        if self.idx >= self.rows.len() {
            return error("EOF");
        }
        self.idx += 1;
        let (k, v) = self
            .rows
            .replace(self.idx - 1, ("".to_string(), Value::Empty));
        Ok(Row::new(vec![Value::from(k), v]))
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
        &STRUCT_STREAM_TYPE
    }
}
