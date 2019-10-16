use crate::data::cell::Cell;

pub struct Argument {
    pub name: String,
    pub cell: Cell,
}

impl Argument {
    pub fn named(name: &String, cell: Cell) -> Argument {
        return Argument {
            name: name.clone(),
            cell: cell,
        };
    }

    pub fn unnamed(cell: Cell) -> Argument {
        return Argument {
            name: String::from(""),
            cell: cell,
        };
    }
}
