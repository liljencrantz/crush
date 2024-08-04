use crate::lang::errors::{CrushResult, data_error};
use std::path::PathBuf;

pub fn cwd() -> CrushResult<PathBuf> {
    std::env::current_dir().map_err(Into::into)
}

pub fn home() -> CrushResult<PathBuf> {
    match dirs::home_dir() {
        None => data_error("Could not find users home directory"),
        Some(p) => Ok(p),
    }
}
