
/**
A type representing a value with a textual representation. It is used in the signature of builtin commands that
accept any type of text value as arguments, e.g. the string matching functions in globs and regexes.
*/
pub enum BinaryInput {
    BinaryInputStream(BinaryInputStream),
    Binary(Arc<[u8]>),
}

impl BinaryInput {
}
