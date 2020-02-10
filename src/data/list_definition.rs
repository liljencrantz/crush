use crate::data::{ValueDefinition, Value, List};
use crate::errors::{error, CrushResult};
use crate::printer::Printer;
use crate::env::Env;
use crate::commands::JobJoinHandle;

#[derive(Clone)]
#[derive(Debug)]
pub struct ListDefinition {
    cells: Vec<ValueDefinition>,
}

impl ListDefinition {
    pub fn new(cells: Vec<ValueDefinition>) -> ListDefinition {
        ListDefinition { cells }
    }

    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> CrushResult<Value> {
        let cells = self.cells
            .iter()
            .map(|c| c.compile(dependencies, env, printer))
            .collect::<CrushResult<Vec<Value>>>()?;
        if cells.len() == 0 {
            return Err(error("Empty list literals not supported"));
        }
        for c in cells.iter() {
            if c.value_type() != cells[0].value_type() {
                return Err(error("All elements in list must be of same type"));
            }
        }
        Ok(Value::List(List::new(
            cells[0].value_type(),
            cells,
        )))
    }
}
