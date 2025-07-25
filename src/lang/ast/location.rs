use crate::lang::errors::{CrushResult, error};
use crate::lang::serialization::model::{Element, element};
use crate::lang::serialization::{DeserializationState, Serializable, SerializationState, model};
use std::cmp::{max, min};

/// A Location tracks the start and end of the definition of something in source code. It is used
/// by jobs, closures, commands, etc in order to be able to give good error reporting.
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

impl Serializable<Location> for Location {
    fn deserialize(
        id: usize,
        elements: &[Element],
        _state: &mut DeserializationState,
    ) -> CrushResult<Location> {
        match elements[id]
            .element
            .as_ref()
            .ok_or(format!("Expected a location"))?
        {
            element::Element::Location(l) => Ok(Location::new(l.start as usize, l.end as usize)),
            _ => error("Expected a location"),
        }
    }

    fn serialize(
        &self,
        elements: &mut Vec<Element>,
        _state: &mut SerializationState,
    ) -> CrushResult<usize> {
        let idx = elements.len();
        elements.push(Element {
            element: Some(element::Element::Location(model::Location {
                start: self.start as u64,
                end: self.end as u64,
            })),
        });
        Ok(idx)
    }
}
