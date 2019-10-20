use crate::data::cell::Cell;
use crate::data::CellDefinition;
use crate::job::Job;
use crate::errors::JobError;

#[derive(Debug)]
pub struct BaseArgument<C> {
    pub name: Option<Box<str>>,
    pub cell: C,
}

pub type ArgumentDefinition = BaseArgument<CellDefinition>;

impl ArgumentDefinition {
    pub fn argument(&self, dependencies: &mut Vec<Job>) -> Result<Argument, JobError> {
        Ok(Argument { name: self.name.clone(), cell: self.cell.clone().cell(dependencies)? })
    }
}

impl Clone for ArgumentDefinition {
    fn clone(&self) -> Self {
        ArgumentDefinition { name: self.name.clone(), cell: self.cell.clone() }
    }
}

impl PartialEq for ArgumentDefinition {
    fn eq(&self, other: &Self) -> bool {
        return self.name == other.name && self.cell == other.cell;
    }
}

pub type Argument = BaseArgument<Cell>;

impl<C> BaseArgument<C> {
    pub fn named(name: &str, cell: C) -> BaseArgument<C> {
        BaseArgument {
            name: Some(Box::from(name)),
            cell: cell,
        }
    }

    pub fn unnamed(cell: C) -> BaseArgument<C> {
        BaseArgument {
            name: None,
            cell: cell,
        }
    }

    pub fn len_or_0(&self) -> usize {
        self.name.as_ref().map(|v| v.len()).unwrap_or(0)
    }

    pub fn val_or_empty(&self) -> &str {
        self.name.as_ref().map(|v| v.as_ref()).unwrap_or("")
    }
}
