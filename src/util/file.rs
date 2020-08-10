use crate::lang::errors::{error, to_crush_error, CrushResult};
use std::path::PathBuf;

pub fn cwd() -> CrushResult<PathBuf> {
    match std::env::current_dir() {
        Ok(d) => Ok(d),
        Err(e) => to_crush_error(Err(e)),
    }
}

pub fn home() -> CrushResult<PathBuf> {
    match dirs::home_dir() {
        Some(d) => Ok(d),
        None => error("Could not find users home directory"),
    }
}
