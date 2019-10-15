use crate::cell::CellType;
use crate::errors::JobError;

pub fn find_field(needle: &String, haystack: &Vec<CellType>) -> Result<usize, JobError> {
    for (idx, field) in haystack.iter().enumerate() {
        if field.name.eq(needle) {
            return Ok(idx);
        }
    }
    return Err(JobError { message: String::from(format!("Unknown column \"{}\"", needle)) });
}
