use std::collections::HashSet;
use std::fmt::Formatter;

pub trait DisplayNonRecursive {
    fn fmt_non_recursive(&self, f: &mut Formatter<'_>, seen: &mut HashSet<u64>) -> std::fmt::Result;
}
