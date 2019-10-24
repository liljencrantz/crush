use crate::data::{CellType, CellDefinition, Cell, List};
use crate::errors::JobError;
use std::hash::Hasher;
use crate::printer::Printer;
use crate::env::Env;
use crate::job::Job;

#[derive(Clone)]
pub struct ListDefinition {
    cell_type: CellType,
    cells: Vec<CellDefinition>,
}

impl ListDefinition {
    pub fn new(cell_type: CellType, cells: Vec<CellDefinition>) -> ListDefinition {
        ListDefinition { cell_type, cells }
    }

    pub fn cell_type(&self) -> CellType {
        self.cell_type.clone()
    }

    pub fn compile(&self, dependencies: &mut Vec<Job>, env: &Env, printer: &Printer) -> Result<Cell, JobError> {
        Ok(Cell::List(List::new(
            self.cell_type.clone(),
            self.cells.iter().map(|c| c.compile(dependencies, env, printer)).collect::<Result<Vec<Cell>, JobError>>()?
        )))
    }
}
