use std::hash::{Hash, Hasher};
use std::collections::hash_map::DefaultHasher;
use std::cmp::max;
use std::fmt::{Display, Formatter};
use SourceIndex::{LookupIndex, ValueIndex};
use std::borrow::Borrow;
use std::ops::Index;

/**
    A simple hash map that preserves insertion order on iteration.
    It uses open hashing, because that is the simplest implementation.
    Some of the performance problems with open hashing is avoided thanks
    to the fact that the whole linked list is implemented via a Vec.

    The lookup buckets only store an integer offset into the value vector,
    meaning that the performance/memory cost of storing very large keys and
    values (including the cost of rehashing) is slightly lessened.

    This struct implements a limited subset of the functionality of the default
    HashMap. The remaining functionality shouldn't be too hard to implement.
*/
pub enum Entry<'a, K: Eq + Hash, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: Eq + Hash, V> Entry<'a, K, V> {
    pub fn insert(self, value: V) {
        match self {
            Entry::Occupied(mut o) => { o.insert(value); }
            Entry::Vacant(v) => { v.insert(value); }
        }
    }
}

enum SourceIndex {
    LookupIndex(usize),
    ValueIndex(usize),
}

pub struct VacantEntry<'a, K: Eq + Hash, V> {
    key: K,
    hash: u64,
    source: SourceIndex,
    map: &'a mut OrderedMap<K, V>,
}

impl<'a, K: Eq + Hash, V> VacantEntry<'a, K, V> {
    pub fn insert(self, value: V) {
        let value_idx = self.map.values.len();
        self.map.values.push(Element::Node(InternalEntry {
            key: self.key,
            value,
            hash: self.hash,
            next_with_same_idx: None,
        }));
        match self.source {
            LookupIndex(lookup_idx) => {
                self.map.lookup[lookup_idx] = Some(value_idx);
            }
            ValueIndex(idx) => {
                match &mut self.map.values[idx] {
                    Element::Node(n) => n.next_with_same_idx = Some(value_idx),
                    Element::Tombstone(t) => t.next_with_same_idx = Some(value_idx),
                }
            }
        }
    }
}

pub struct OccupiedEntry<'a, K: Eq + Hash, V> {
    map: &'a mut OrderedMap<K, V>,
    index: usize,
}

impl<'a, K: Eq + Hash, V> OccupiedEntry<'a, K, V> {
    pub fn key(&self) -> &K {
        match &self.map.values[self.index] {
            Element::Node(n) => &n.key,
            Element::Tombstone(_) => panic!("AAAA"),
        }
    }

    pub fn value(&self) -> &V {
        match &self.map.values[self.index] {
            Element::Node(n) => &n.value,
            Element::Tombstone(_) => panic!("AAAA"),
        }
    }

    pub fn remove(self) -> V {
        let idx;
        self.map.tombstones += 1;
        match &mut self.map.values[self.index] {
            Element::Node(n) => {
                idx = n.next_with_same_idx;
            }
            Element::Tombstone(_) => { panic!("AAAA") }
        }
        let mut el = Element::Tombstone(Tombstone {
            next_with_same_idx: idx,
        });
        std::mem::swap(&mut el, &mut self.map.values[self.index]);
        match el {
            Element::Node(n) => n.value,
            Element::Tombstone(_) => panic!("AAAA"),
        }
    }

    pub fn insert(&mut self, value: V) -> V {
        match &mut self.map.values[self.index] {
            Element::Node(n) => std::mem::replace(&mut n.value, value),
            Element::Tombstone(_) => panic!("AAAA"),
        }
    }
}

#[derive(Debug)]
struct InternalEntry<K: Eq + Hash, V> {
    key: K,
    value: V,
    hash: u64,
    next_with_same_idx: Option<usize>,
}

impl<K, V> Clone for InternalEntry<K, V>
    where
        K: Eq + Hash + Clone,
        V: Clone
{
    fn clone(&self) -> Self {
        InternalEntry {
            key: self.key.clone(),
            value: self.value.clone(),
            hash: self.hash,
            next_with_same_idx: self.next_with_same_idx,
        }
    }
}


#[derive(Debug, Clone)]
struct Tombstone {
    next_with_same_idx: Option<usize>,
}

#[derive(Debug)]
enum Element<K: Eq + Hash, V> {
    Node(InternalEntry<K, V>),
    Tombstone(Tombstone),
}

impl<K, V> Clone for Element<K, V>
    where
        K: Eq + Hash + Clone,
        V: Clone
{
    fn clone(&self) -> Self {
        match self {
            Element::Node(n) => Element::Node(n.clone()),
            Element::Tombstone(t) => Element::Tombstone(t.clone()),
        }
    }
}

#[derive(Debug)]
pub struct OrderedMap<K: Eq + Hash, V> {
    lookup: Vec<Option<usize>>,
    values: Vec<Element<K, V>>,
    tombstones: usize,
}

impl<K: Eq + Hash, V> OrderedMap<K, V> {
    pub fn new() -> OrderedMap<K, V> {
        OrderedMap::with_capacity(8)
    }

    pub fn with_capacity(capacity: usize) -> OrderedMap<K, V> {
        OrderedMap {
            lookup: vec![None; capacity],
            values: Vec::with_capacity(capacity),
            tombstones: 0,
        }
    }

    pub fn capacity(&self) -> usize {
        self.lookup.capacity()
    }

    pub fn len(&self) -> usize {
        self.values.len() - self.tombstones
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn reallocate(&mut self, mut new_capacity: usize) {
        new_capacity = max(new_capacity, 1);
        self.lookup = vec![None; new_capacity];
        if self.tombstones == 0 {
            self.values.reserve(new_capacity - self.values.len());
        } else {
            self.tombstones = 0;
            let mut replacement: Vec<Element<K, V>> = Vec::with_capacity(new_capacity);
            for el in self.values.drain(..) {
                match el {
                    Element::Node(n) => replacement.push(Element::Node(n)),
                    Element::Tombstone(_) => {}
                }
            }
            self.values = replacement;
        }
        for i in 0..self.values.len() {
            let el = &mut self.values[i];
            match el {
                Element::Node(n) => { n.next_with_same_idx = None }
                Element::Tombstone(t) => { t.next_with_same_idx = None }
            }
            self.insert_into_lookup(i);
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.entry(key).insert(value);
    }

    fn insert_into_lookup(&mut self, value_idx: usize) {
        match &mut self.values[value_idx] {
            Element::Node(node) => {
                let lookup_idx = (node.hash as usize) % self.lookup.len();

                match self.lookup[lookup_idx] {
                    None => {
                        self.lookup[lookup_idx] = Some(value_idx);
                    }
                    Some(mut prev_with_same_idx) => {
                        loop {
                            match &self.values[prev_with_same_idx] {
                                Element::Node(n) => {
                                    match n.next_with_same_idx {
                                        None => break,
                                        Some(idx) => prev_with_same_idx = idx,
                                    }
                                }
                                Element::Tombstone(t) => {
                                    match t.next_with_same_idx {
                                        None => break,
                                        Some(idx) => prev_with_same_idx = idx,
                                    }
                                }
                            }
                        }
                        match &mut self.values[prev_with_same_idx] {
                            Element::Node(n) => {
                                n.next_with_same_idx = Some(value_idx);
                            }
                            Element::Tombstone(t) => {
                                t.next_with_same_idx = Some(value_idx);
                            }
                        }
                    }
                }
            }
            Element::Tombstone(_) => {}
        }
    }

    pub fn get<Q: ?Sized>(&self, key: &Q) -> Option<&V>
        where
            K: Borrow<Q>,
            Q: Hash + Eq, {
        match self.find(key) {
            Err(_) => None,
            Ok(idx) => match &self.values[idx] {
                Element::Node(n) => Some(&n.value),
                Element::Tombstone(_) => panic!("Invalid result for find operation"),
            },
        }
    }

    pub fn contains_key<Q: ?Sized>(&self, key: &Q) -> bool
        where
            K: Borrow<Q>,
            Q: Hash + Eq, {
        match self.find(key) {
            Err(_) => false,
            Ok(idx) => match &self.values[idx] {
                Element::Node(_) => true,
                Element::Tombstone(_) => panic!("Invalid result for find operation"),
            },
        }
    }

    pub fn remove<Q: ?Sized>(&mut self, key: &Q) -> Option<V>
        where
            K: Borrow<Q>,
            Q: Hash + Eq, {
        match self.find(key) {
            Err(_) => None,
            Ok(idx) => {
                self.tombstones += 1;
                let next_with_same_idx = match &self.values[idx] {
                    Element::Node(n) => n.next_with_same_idx,
                    Element::Tombstone(t) => t.next_with_same_idx,
                };
                let mut el = Element::Tombstone::<K, V>(Tombstone { next_with_same_idx });
                std::mem::swap(&mut el, &mut self.values[idx]);
                match el {
                    Element::Node(n) => {
                        Some(n.value)
                    }
                    Element::Tombstone(_) => panic!("Impossible"),
                }
            }
        }
    }

    fn find<Q: ?Sized>(&self, key: &Q) -> Result<usize, SourceIndex>
        where
            K: Borrow<Q>,
            Q: Hash + Eq, {
        let hash = self.hash(&key);
        self.find_from_hash(key, hash)
    }

    fn find_from_hash<Q: ?Sized>(&self, key: &Q, hash: u64) -> Result<usize, SourceIndex>
        where
            K: Borrow<Q>,
            Q: Hash + Eq, {
        let lookup_idx = (hash as usize) % self.lookup.len();
        match self.lookup[lookup_idx] {
            None => Err(SourceIndex::LookupIndex(lookup_idx)),
            Some(mut prev_with_same_idx) => {
                loop {
                    match &self.values[prev_with_same_idx] {
                        Element::Node(n) => {
                            if n.key.borrow().eq(&key) {
                                return Ok(prev_with_same_idx);
                            }
                            match n.next_with_same_idx {
                                None => return Err(SourceIndex::ValueIndex(prev_with_same_idx)),
                                Some(idx) => {
                                    prev_with_same_idx = idx
                                }
                            }
                        }
                        Element::Tombstone(t) => {
                            match t.next_with_same_idx {
                                None => return Err(SourceIndex::ValueIndex(prev_with_same_idx)),
                                Some(idx) => {
                                    prev_with_same_idx = idx
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn hash<Q: ?Sized>(&self, key: &Q) -> u64
        where
            K: Borrow<Q>,
            Q: Hash + Eq, {
        let mut s = DefaultHasher::new();
        key.hash(&mut s);
        s.finish()
    }

    pub fn entry(&mut self, key: K) -> Entry<K, V> {
        if self.capacity() <= (self.len() + self.tombstones) {
            self.reallocate(self.capacity() * 2);
        }
        let hash = self.hash(&key);

        match self.find_from_hash(&key, hash) {
            Ok(index) =>
                Entry::Occupied(OccupiedEntry {
                    index,
                    map: self,
                }),

            Err(source) =>
                Entry::Vacant(VacantEntry {
                    key,
                    hash,
                    source,
                    map: self,
                }),
        }
    }

    pub fn iter(&self) -> Iter<K, V> {
        Iter {
            liter: self.values.iter(),
        }
    }

    pub fn keys(&self) -> Keys<K, V> {
        Keys {
            liter: self.values.iter(),
        }
    }

    pub fn values(&self) -> Values<K, V> {
        Values {
            liter: self.values.iter(),
        }
    }

    pub fn iter_mut(&mut self) -> IterMut<K, V> {
        IterMut {
            liter: self.values.iter_mut(),
        }
    }

    pub fn clear(&mut self) {
        self.tombstones = 0;
        self.values.clear();
        self.lookup.clear();
    }

    pub fn drain(&mut self) -> Drain<K, V> {
        self.tombstones = 0;
        self.lookup.drain(..);
        Drain {
            liter: self.values.drain(..),
        }
    }
}

impl<K, V> Clone for OrderedMap<K, V>
    where
        K: Eq + Hash + Clone,
        V: Clone
{
    fn clone(&self) -> Self {
        OrderedMap {
            lookup: self.lookup.clone(),
            values: self.values.clone(),
            tombstones: self.tombstones,
        }
    }
}

impl<K, V> std::iter::FromIterator<(K, V)> for OrderedMap<K, V>
    where
        K: Eq + Hash,
{
    fn from_iter<T: IntoIterator<Item=(K, V)>>(iter: T) -> OrderedMap<K, V> {
        let mut map = OrderedMap::new();
        map.extend(iter);
        map
    }
}

impl<K, V> Extend<(K, V)> for OrderedMap<K, V>
    where
        K: Eq + Hash,
{
    fn extend<T: IntoIterator<Item=(K, V)>>(&mut self, iter: T) {
        let mut i = iter.into_iter();
        loop {
            match i.next() {
                None => break,
                Some((key, value)) => self.insert(key, value),
            }
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
        for (key, val) in self.iter() {
            if first {
                first = false;
            } else {
                f.write_str(", ")?;
            }
            key.fmt(f)?;
            f.write_str(": ")?;
            val.fmt(f)?;
        }
        f.write_str("]")?;
        Ok(())
    }
}

pub struct Iter<'a, K: Eq + Hash, V> {
    liter: std::slice::Iter<'a, Element<K, V>>,
}

impl<'a, K: Eq + Hash, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.liter.next() {
                None => return None,
                Some(Element::Tombstone(_)) => {}
                Some(Element::Node(n)) => return Some((&n.key, &n.value)),
            }
        }
    }
}

pub struct Keys<'a, K: Eq + Hash, V> {
    liter: std::slice::Iter<'a, Element<K, V>>,
}

impl<'a, K: Eq + Hash, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.liter.next() {
                None => return None,
                Some(Element::Tombstone(_)) => {}
                Some(Element::Node(n)) => return Some(&n.key),
            }
        }
    }
}

pub struct Values<'a, K: Eq + Hash, V> {
    liter: std::slice::Iter<'a, Element<K, V>>,
}

impl<'a, K: Eq + Hash, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.liter.next() {
                None => return None,
                Some(Element::Tombstone(_)) => {}
                Some(Element::Node(n)) => return Some(&n.value),
            }
        }
    }
}

pub struct Drain<'a, K: Eq + Hash, V> {
    liter: std::vec::Drain<'a, Element<K, V>>,
}

impl<'a, K: Eq + Hash, V> Iterator for Drain<'a, K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.liter.next() {
                None => return None,
                Some(Element::Tombstone(_)) => {}
                Some(Element::Node(n)) => return Some((n.key, n.value)),
            }
        }
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
    liter: std::slice::IterMut<'a, Element<K, V>>,
}

impl<'a, K: Eq + Hash, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.liter.next() {
                None => return None,
                Some(Element::Tombstone(_)) => {}
                Some(Element::Node(n)) => return Some((&n.key, &mut n.value)),
            }
        }
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
    liter: std::vec::IntoIter<Element<K, V>>,
}

impl<K: Eq + Hash, V> Iterator for IntoIter<K, V> {
    type Item = (K, V);

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.liter.next() {
                None => return None,
                Some(Element::Tombstone(_)) => {}
                Some(Element::Node(n)) => return Some((n.key, n.value)),
            }
        }
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

impl<'a, K, Q: ?Sized, V> Index<&'a Q> for OrderedMap<K, V>
    where
        K: Eq + Hash + Borrow<Q>,
        Q: Eq + Hash,
{
    type Output = V;

    fn index(&self, key: &Q) -> &V {
        self.get(key).unwrap()
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
        assert_eq!(m.len(), 3);
        assert_eq!(m.iter().map(|(_, v)| v.to_string()).collect::<String>(), "acb".to_string());
    }

    #[test]
    fn test_replacement() {
        let mut m = OrderedMap::new();
        m.insert(1, "a");
        m.insert(1, "c");
        m.insert(1, "b");
        assert_eq!(m.len(), 1);
        assert_eq!(m.get(&1).unwrap(), &"b");
    }

    #[test]
    fn test_remove() {
        let mut m = OrderedMap::new();
        m.insert(1, "a");
        m.insert(3, "c");
        m.insert(4, "d");
        m.insert(2, "b");

        assert_eq!(m.len(), 4);

        m.remove(&1);
        assert_eq!(m.len(), 3);
        assert_eq!(m.get(&1), None);

        assert_eq!(m.to_string(), "[3: c, 4: d, 2: b]");
    }

    #[test]
    fn test_entry_remove() {
        let mut m = OrderedMap::new();
        m.insert(1, "a");
        m.insert(3, "c");
        m.insert(4, "d");
        m.insert(2, "b");

        assert_eq!(m.len(), 4);

        match m.entry(1) {
            Entry::Occupied(e) => { e.remove(); }
            Entry::Vacant(_) => { panic!("Impossible"); }
        }
        assert_eq!(m.len(), 3);
        assert_eq!(m.get(&1), None);

        assert_eq!(m.to_string(), "[3: c, 4: d, 2: b]");
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
            m.insert(i, i + 1);
        }
        for i in 0..10000 {
            assert_eq!(m.get(&i).unwrap(), &(i + 1));
        }
    }

    #[test]
    fn test_with_realloc_and_overwrite() {
        let mut m = OrderedMap::new();
        for i in 0..10000 {
            m.insert(i, i + 1);
        }
        for i in (0..10000).step_by(2) {
            m.insert(i, i + 2);
        }
        for i in (0..10000).step_by(2) {
            assert_eq!(m.get(&i).unwrap(), &(i + 2));
        }
        for i in (1..10000).step_by(2) {
            assert_eq!(m.get(&i).unwrap(), &(i + 1));
        }
    }

    #[test]
    fn test_with_realloc_and_remove_and_overwrite() {
        let mut m = OrderedMap::new();
        for i in 0..10000 {
            m.insert(i, i + 1);
        }
        for i in (0..10000).step_by(2) {
            m.remove(&i);
        }
        for i in (0..10000).step_by(4) {
            m.insert(i, i + 2);
        }
        for i in (0..10000).step_by(4) {
            assert_eq!(m.get(&i).unwrap(), &(i + 2));
        }
        for i in (1..10000).step_by(2) {
            assert_eq!(m.get(&i).unwrap(), &(i + 1));
        }
        for i in (2..10000).step_by(4) {
            assert_eq!(m.get(&i), None);
        }
    }

    #[test]
    fn test_for_loop() {
        let mut m = OrderedMap::new();
        m.insert(1, "a".to_string());
        m.insert(3, "c".to_string());
        m.insert(2, "b".to_string());
        for (_, v) in &mut m {
            v.push_str(".")
        }
        let mut r = "".to_string();
        for (_, v) in &m {
            r.push_str(&v);
        }
        assert_eq!(&r, "a.c.b.");

        let mut r2 = "".to_string();
        for (_, v) in m {
            r2.push_str(&v);
        }
        assert_eq!(&r2, "a.c.b.");
    }
}
