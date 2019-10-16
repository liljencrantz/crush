use crate::data::{CellType};
use crate::errors::JobError;

pub fn find_field(needle: &String, haystack: &Vec<CellType>) -> Result<usize, JobError> {
    for (idx, field) in haystack.iter().enumerate() {
        if field.name.eq(needle) {
            return Ok(idx);
        }
    }

    return Err(
        JobError {
            message: format!(
                "Unknown column {}, available columns are {}",
                needle,
                haystack.iter().map(|t| t.name.clone()).collect::<Vec<String>>().join(", "),
            )
        }
    );
}
