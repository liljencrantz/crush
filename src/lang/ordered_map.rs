use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::cmp::max;
use std::fmt::{Display, Formatter};

/**
    A simple hash map that preserves insertion order on iteration.
    It uses open hashing, because that is the simplest implementation.
    Some of the performance problems with open hashing is avoided thanks
    to the fact that the whole linked list is implemented via a Vec.

    The lookup buckets only store an integer offset into the value vector,
    meaning that the performance/memory cost of storing very large keys and
    values (including the cost of rehashing) is slightly lessened.

    Element removal has not been implemented, because it's currently not needed.
    Also, it would require tombstones and overall just make thing more complicated.
*/

#[derive(Debug)]
struct Node<K: Eq + Hash, V> {
    key: K,
    value: V,
    hash: u64,
    next_with_same_idx: Option<usize>,
}

#[derive(Debug)]
pub struct OrderedMap<K: Eq + Hash, V> {
    lookup: Vec<Option<usize>>,
    values: Vec<Node<K, V>>,
}

impl<K: Eq + Hash, V> OrderedMap<K, V> {
    pub fn new() -> OrderedMap<K, V> {
        OrderedMap::with_capacity(8)
    }

    pub fn with_capacity(capacity: usize) -> OrderedMap<K, V> {
        OrderedMap {
            lookup: vec![None; capacity],
            values: Vec::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.lookup.capacity()
    }

    pub fn len(&self) -> usize {
        self.values.len()
    }


    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn reallocate( &mut self, mut new_capacity: usize) {
        new_capacity = max(new_capacity, 1);
        self.lookup = vec![None; new_capacity];
        for i in 0..self.values.len() {
            self.values[i].next_with_same_idx = None;
            self.insert_into_lookup(i);
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        if self.capacity() <= self.len() {
            self.reallocate(self.capacity()*2);
        }

        let mut s = DefaultHasher::new();
        key.hash(&mut s);
        let hash = s.finish();

        let value_idx = self.values.len();
        self.values.push(Node {
            key,
            value,
            hash,
            next_with_same_idx: None
        });

        self.insert_into_lookup(value_idx);
    }

    fn insert_into_lookup(&mut self, value_idx: usize) {
        let lookup_idx = (self.values[value_idx].hash as usize) % self.lookup.len();

        match self.lookup[lookup_idx] {
            None => {
                self.lookup[lookup_idx] = Some(value_idx);
            },
            Some(mut prev_with_same_idx) => {
                loop {
                    match self.values[prev_with_same_idx].next_with_same_idx {
                        None => break,
                        Some(idx) => prev_with_same_idx = idx,
                    }
                }
                self.values[prev_with_same_idx].next_with_same_idx = Some(value_idx);
            },
        }
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        let mut s = DefaultHasher::new();
        key.hash(&mut s);
        let hash = s.finish();
        let lookup_idx = (hash as usize) % self.lookup.len();
        match self.lookup[lookup_idx] {
            None => None,
            Some(mut prev_with_same_idx) => {
                loop {
                    if &self.values[prev_with_same_idx].key == key {
                        return Some(&self.values[prev_with_same_idx].value)
                    }
                    match self.values[prev_with_same_idx].next_with_same_idx {
                        None => return None,
                        Some(idx) => {
                            prev_with_same_idx = idx
                        },
                    }
                }
            },
        }

    }

    pub fn iter(&self) -> Iter<K, V>{
        Iter {
            liter: self.values.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V>{
        IterMut {
            liter: self.values.iter_mut(),
        }
    }
}

impl<K: Eq + Hash, V> Default for OrderedMap<K, V> {
    fn default() -> Self {
        OrderedMap::new()
    }
}

impl<K: Eq + Hash + Display, V: Display> Display for OrderedMap<K, V> {

    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str("[")?;
        let mut first = true;
        for n in self.values.iter() {
            if first {
                first = false;
            } else {
                f.write_str(", ")?;
            }
            n.key.fmt(f)?;
            f.write_str(": ")?;
            n.value.fmt(f)?;
        }
        f.write_str("]")?;
        Ok(())
    }
}

pub struct Iter<'a, K: Eq + Hash, V> {
    liter: std::slice::Iter<'a, Node<K, V>>,
}

impl<'a, K: Eq + Hash, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        self.liter.next().map(|n| (&n.key, &n.value))
    }
}

impl<'a, K: Eq + Hash, V> IntoIterator for &'a OrderedMap<K, V> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            liter: self.values.iter(),
        }
    }
}

pub struct IterMut<'a, K: Eq + Hash, V> {
    liter: std::slice::IterMut<'a, Node<K, V>>,
}

impl<'a, K: Eq + Hash, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        self.liter.next().map(|n| (&n.key, &mut n.value))
    }
}

impl<'a, K: Eq + Hash, V> IntoIterator for &'a mut OrderedMap<K, V> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IterMut {
            liter: self.values.iter_mut(),
        }
    }
}

pub struct IntoIter<K: Eq + Hash, V> {
    liter: std::vec::IntoIter<Node<K, V>>,
}

impl<K: Eq + Hash, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        self.liter.next().map(|n| (n.key, n.value))
    }
}

impl<K: Eq + Hash, V> IntoIterator for OrderedMap<K, V> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            liter: self.values.into_iter(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_map() {
        let mut m = OrderedMap::new();
        m.insert(1, "a");
        m.insert(3, "c");
        m.insert(2, "b");
        assert_eq!(m.get(&1).unwrap(), &"a");
        assert_eq!(m.get(&2).unwrap(), &"b");
        assert_eq!(m.get(&3).unwrap(), &"c");
        assert_eq!(m.iter().map(|(k, v)| v.to_string()).collect::<String>(), "acb".to_string());
    }

    #[test]
    fn test_fmt() {
        let mut m = OrderedMap::new();
        m.insert(1, "a");
        m.insert(3, "c");
        m.insert(2, "b");
        assert_eq!(m.to_string(), "[1: a, 3: c, 2: b]");
    }

    #[test]
    fn test_with_realloc() {
        let mut m = OrderedMap::new();
        for i in 0..10000 {
            m.insert(i, i+1);
        }
        for i in 0..10000 {
            assert_eq!(m.get(&i).unwrap(), &(i+1));
        }
    }

    #[test]
    fn test_for_loop() {
        let mut m = OrderedMap::new();
        m.insert(1, "a".to_string());
        m.insert(3, "c".to_string());
        m.insert(2, "b".to_string());
        for (k, v) in &mut m {
            v.push_str(".")
        }
        let mut r = "".to_string();
        for (k, v) in &m {
            r.push_str(&v);
        }
        assert_eq!(&r, "a.c.b.");

        let mut r2 = "".to_string();
        for (k, v) in m {
            r2.push_str(&v);
        }
        assert_eq!(&r2, "a.c.b.");
    }
}
