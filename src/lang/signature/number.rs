/**
A type representing a f64 or an i128. It is used in the signature of builtin commands that
accept any type of nomeric value as arguments, e.g. the math library.
*/
pub enum Number {
    Float(f64),
    Integer(i128),
}

impl Number {
    pub fn as_float(&self) -> f64 {
        match self {
            Number::Float(f) => *f,
            Number::Integer(i) => *i as f64,
        }
    }
}
