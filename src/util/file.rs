use crate::lang::errors::CrushResult;
use std::path::PathBuf;

pub fn cwd() -> CrushResult<PathBuf> {
    std::env::current_dir().map_err(Into::into)
}

pub fn home() -> CrushResult<PathBuf> {
    dirs::home_dir().ok_or_else(|| "Could not find users home directory".into())
}
