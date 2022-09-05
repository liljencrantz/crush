use std::fmt::{Display, Formatter};
use crate::lang::ast::location::Location;

#[derive(Clone, Debug)]
pub struct TrackedString {
    pub string: String,
    pub location: Location,
}

impl TrackedString {
    pub fn from(string: &str, location: Location) -> TrackedString {
        TrackedString {
            string: string.to_string(),
            location,
        }
    }

    pub fn literal(start: usize, string: &str, end: usize) -> TrackedString {
        TrackedString {
            string: string.to_string(),
            location: Location::new(start, end),
        }
    }

    pub fn prefix(&self, pos: usize) -> TrackedString {
        if !self.location.contains(pos) {
            if self.location.start > pos {
                TrackedString {
                    string: "".to_string(),
                    location: Location::new(self.location.start, self.location.start),
                }
            } else {
                self.clone()
            }
        } else {
            let len = pos - self.location.start;
            TrackedString {
                string: self.string[0..len].to_string(),
                location: Location::new(self.location.start, self.location.start + len),
            }
        }
    }
}

impl Display for TrackedString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.string)
    }
}
