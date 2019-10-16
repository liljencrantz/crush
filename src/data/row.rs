use crate::data::cell::Cell;
use std::cmp::Ordering;
use std::cmp::PartialOrd;
use std::hash::Hasher;
use crate::data::CellType;

#[derive(Debug)]
pub struct Row {
    pub cells: Vec<Cell>,
}

impl Row {
    pub fn concrete(&self) -> Self {
        Row {cells: self.cells.iter().map(|c| c.concrete()).collect()}
    }
}

impl std::hash::Hash for Row {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for c in &self.cells {
            c.hash(state);
        }
    }
}

#[derive(Debug)]
pub struct RowWithTypes {
    pub types: Vec<CellType>,
    pub cells: Vec<Cell>,
}

impl std::hash::Hash for RowWithTypes {
    fn hash<H: Hasher>(&self, state: &mut H) {
        for c in &self.cells {
            c.hash(state);
        }
    }
}

impl Clone for RowWithTypes {
    fn clone(&self) -> Self {
        RowWithTypes {
            types: self.types.clone(),
            cells: self.cells.iter().map(|c| c.concrete()).collect(),
        }
    }
}

impl PartialOrd for RowWithTypes {
    fn partial_cmp(&self, other: &RowWithTypes) -> Option<Ordering> {
        if self.cells.len() != other.cells.len() {
            return Some(self.cells.len().cmp(&other.cells.len()));
        }

        let mut res = Ordering::Equal;
        for (l, r) in self.cells.iter().zip(other.cells.iter()) {
            match l.partial_cmp(r) {
                None => return None,
                Some(ordering) => match ordering {
                    Ordering::Equal => {}
                    Ordering::Greater | Ordering::Less => {
                        res = ordering;
                        break;
                    },
                },
            }
        }
        return Some(res);
    }
}

impl std::cmp::PartialEq for RowWithTypes {
    fn eq(&self, other: &RowWithTypes) -> bool {
        if self.cells.len() != other.cells.len() {
            return false;
        }

        let mut res = false;
        for (l, r) in self.cells.iter().zip(other.cells.iter()) {
            match l.eq(r) {
                true => {},
                false => {
                    res = false;
                    break;
                }
            }
        }
        return res;
    }
}
