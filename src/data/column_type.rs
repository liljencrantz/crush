use crate::data::CellType;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ColumnType {
    pub name: Option<Box<str>>,
    pub cell_type: CellType,
}

impl ColumnType {

    pub fn to_string(&self) -> String {
        match &self.name {
            None => self.cell_type.to_string(),
            Some(n) => format!("{}={}", n, self.cell_type.to_string()),
        }
    }

    pub fn named(name: &str, cell_type: CellType) -> ColumnType {
        ColumnType {
            name: Some(Box::from(name)),
            cell_type,
        }
    }

    pub fn len_or_0(&self) -> usize {
        self.name.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    pub fn val_or_empty(&self) -> &str {
        self.name.as_ref().map(|v| v.as_ref()).unwrap_or("")
    }
}
