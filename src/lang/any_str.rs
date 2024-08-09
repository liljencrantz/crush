use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/**
    An enum representing some type of string value. Can be used in situations where you want to
    either use a string literal or a String instance and you're not OK with turning the literal into
    a string, e.g. because you want to use a const function.
*/
#[derive(Clone, Debug, Eq, Ord)]
pub enum AnyStr {
    Slice(&'static str),
    String(String),
}

impl From<&'static str> for AnyStr {
    fn from(value: &'static str) -> Self {
        AnyStr::Slice(value)
    }
}

impl From<String> for AnyStr {
    fn from(value: String) -> Self {
        AnyStr::String(value)
    }
}

impl AnyStr {
    pub fn to_string(&self) -> String {
        match self {
            AnyStr::Slice(s) => s.to_string(),
            AnyStr::String(s) => s.clone(),
        }
    }

    pub fn to_str(&self) -> &str {
        match self {
            AnyStr::Slice(s) => s,
            AnyStr::String(s) => &s,
        }
    }
}

impl PartialEq for AnyStr {
    fn eq(&self, other: &Self) -> bool {
        self.to_str().eq(other.to_str())
    }
}

impl PartialOrd for AnyStr {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_str().partial_cmp(other.to_str())
    }
}

impl Hash for AnyStr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_str().hash(state)
    }
}
