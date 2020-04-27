use std::collections::HashMap;

#[derive(Debug)]
pub struct OrderedStringMap <T> {
    mapping: HashMap<String, usize>,
    values: Vec<T>,
}

impl <T> OrderedStringMap <T> {
    pub fn new() -> OrderedStringMap<T> {
        OrderedStringMap {
            mapping: HashMap::new(),
            values: Vec::new(),
        }
    }

    pub fn insert(&mut self, key: String, value: T) {
        let idx = self.values.len();
        self.values.push(value);
        self.mapping.insert(key, idx);
    }

    pub fn get(&self, key: &str) -> Option<&T> {
        self.mapping.get(key).map(|idx| &self.values[*idx])
    }

    pub fn iter(&self) -> std::slice::Iter<'_, T>{
        self.values.iter()
    }
}