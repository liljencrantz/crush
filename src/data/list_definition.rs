use crate::data::{CellType, CellDefinition, Cell, List};
use crate::errors::JobError;
use std::hash::Hasher;
use crate::printer::Printer;
use crate::env::Env;
use crate::job::Job;
use crate::commands::JobJoinHandle;

#[derive(Clone)]
pub struct ListDefinition {
    cells: Vec<CellDefinition>,
}

impl ListDefinition {
    pub fn new(cells: Vec<CellDefinition>) -> ListDefinition {
        ListDefinition { cells }
    }

    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> Result<Cell, JobError> {
        let cells = self.cells
            .iter()
            .map(|c| c.compile(dependencies, env, printer))
            .collect::<Result<Vec<Cell>, JobError>>()?;
        Ok(Cell::List(List::new(
            cells[0].cell_type(),
            cells,
        )))
    }
}
