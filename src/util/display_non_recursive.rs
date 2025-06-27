use std::collections::HashSet;
use std::fmt::Formatter;

/// Version of the Display trait that avoids infinite recursion in the case of self-referencing 
/// elements by checking the memory address of each object.
/// 
/// Whenever infinite recursion is detected, print "..." instead of the infinite value.
/// 
/// Using object identity instead of a real hash may be seen as an ugly crutch, but the objects
/// that can contain self references, can all also be mutated, meaning there is no stable way
/// to calculate a hashcode and equality for a hash set other than assigning an identity to each
/// object. Either we do that via taking the pointer value or via assigning an identity to each
/// object upon creation. The latter would also work, but would add decent extra memory overhead.
pub trait DisplayNonRecursive {
    fn fmt_non_recursive(&self, f: &mut Formatter<'_>, seen: &mut HashSet<u64>)
    -> std::fmt::Result;
}
