use crate::data::cell::Cell;

pub struct Argument {
    pub name: Option<Box<str>>,
    pub cell: Cell,
}

impl Argument {
    pub fn named(name: &str, cell: Cell) -> Argument {
        return Argument {
            name: Some(Box::from(name)),
            cell: cell,
        };
    }

    pub fn unnamed(cell: Cell) -> Argument {
        return Argument {
            name: None,
            cell: cell,
        };
    }

    pub fn len_or_0(&self) -> usize {
        self.name.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    pub fn val_or_empty(&self) -> &str {
        self.name.as_ref().map(|v| v.as_ref()).unwrap_or("")
    }
}
