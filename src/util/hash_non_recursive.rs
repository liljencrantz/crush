use std::collections::HashSet;
use std::hash::Hasher;

pub trait HashNonRecursive {
    fn hash_non_recursive<H: Hasher>(&self, state: &mut H, seen: &mut HashSet<u64>);
}
