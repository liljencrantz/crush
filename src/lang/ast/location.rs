use std::cmp::{max, min};

/// A Location tracks the start and end of the definition of something in source code. It is used
/// by jobs, closures, commands, etc. in order to be able to give good error reporting.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub struct Location {
    pub start: usize,
    pub end: usize,
}

impl Location {
    pub fn new(start: usize, end: usize) -> Location {
        Location { start, end }
    }

    pub fn union(&self, other: Location) -> Location {
        Location {
            start: min(self.start, other.start),
            end: max(self.end, other.end),
        }
    }

    pub fn contains(&self, cursor: usize) -> bool {
        cursor >= self.start && cursor <= self.end
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }
}

impl From<usize> for Location {
    fn from(value: usize) -> Self {
        Location {
            start: value,
            end: value + 1,
        }
    }
}
