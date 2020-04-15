use crate::lang::errors::{CrushResult, error, to_crush_error};
use std::path::Path;

pub fn cwd() -> CrushResult<Box<Path>> {
    match std::env::current_dir() {
        Ok(d) => Ok(d.into_boxed_path()),
        Err(e) => to_crush_error(Err(e)),
    }
}

pub fn home() -> CrushResult<Box<Path>> {
    match dirs::home_dir() {
        Some(d) => Ok(d.into_boxed_path()),
        None => error("Could not find users home directory"),
    }
}
