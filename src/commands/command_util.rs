use crate::data::{CellType};
use crate::errors::JobError;

pub fn find_field(needle: &String, haystack: &Vec<CellType>) -> Result<usize, JobError> {
    for (idx, field) in haystack.iter().enumerate() {
        if field.name.as_ref().map(|v| v.eq(needle)).unwrap_or(false) {
            return Ok(idx);
        }
    }

    return Err(
        JobError {
            message: format!(
                "Unknown column {}, available columns are {}",
                needle,
                haystack.iter().map(|t| t.val_or_empty().to_string()).collect::<Vec<String>>().join(", "),
            )
        }
    );
}
