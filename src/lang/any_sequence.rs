use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::Deref;

/**
    An enum representing some type of sequence of values. Can be used in situations where you want
    to either use a static slice or a Vec instance and you're not OK with turning the slice into a
    Vec, e.g. because you want to use a const function.
*/
#[derive(Clone, Debug, Eq)]
pub enum AnySequence<T: 'static> {
    Slice(&'static [T]),
    Vec(Vec<T>),
}

impl<T: 'static> From<&'static [T]> for AnySequence<T> {
    fn from(value: &'static [T]) -> Self {
        AnySequence::Slice(value)
    }
}

impl<T: 'static> From<Vec<T>> for AnySequence<T> {
    fn from(value: Vec<T>) -> Self {
        AnySequence::Vec(value)
    }
}

impl<T: 'static> AnySequence<T> {
    pub fn as_slice(&self) -> &[T] {
        match self {
            AnySequence::Slice(s) => s,
            AnySequence::Vec(v) => v,
        }
    }
}

impl<T: 'static + PartialEq> PartialEq for AnySequence<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice().eq(other.as_slice())
    }
}

impl<T: 'static + PartialOrd> PartialOrd for AnySequence<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: 'static + Ord> Ord for AnySequence<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: 'static + Hash> Hash for AnySequence<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}

impl<T: 'static> Deref for AnySequence<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}
