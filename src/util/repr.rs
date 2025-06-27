use std::fmt::Formatter;

/// A trait for types that can be represented as a string.
/// Different from Display only in intent: Display is meant to be a descriptive
/// representation of some value, whereas Repr is meant to be a representation
/// that can be interpreted into the value again by a computer program.
pub trait Repr {
    fn repr(&self, f: &mut Formatter<'_>) -> std::fmt::Result;
}
