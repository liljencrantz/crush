use crate::data::{CellDefinition, Cell, List};
use crate::errors::{JobError, argument_error, error};
use crate::printer::Printer;
use crate::env::Env;
use crate::commands::JobJoinHandle;

#[derive(Clone)]
#[derive(Debug)]
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
        if cells.len() == 0 {
            return Err(error("Empty list literals not supported"));
        }
        for c in cells.iter() {
            if c.cell_type() != cells[0].cell_type() {
                return Err(error("All elements in list must be of same type"));
            }
        }
        Ok(Cell::List(List::new(
            cells[0].cell_type(),
            cells,
        )))
    }
}
