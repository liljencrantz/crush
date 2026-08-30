use std::sync::Mutex;
use crate::lang::data::table::Row;
use crate::lang::errors::{command_error, CrushResult};
use crate::lang::value::Value;

static ENV_LOCK: Mutex<()> = Mutex::new(());

pub fn get(name: &str) -> CrushResult<String> {
    let _lock = ENV_LOCK.lock();
    Ok(std::env::var(name)?)
}

pub fn set(name: &str, value: &str) -> CrushResult<()> {

    if name == "" || name.contains('=') || name.contains('\0') {
        return command_error("Invalid environment variable name");
    }

    if value.contains('\0') {
        return command_error("Invalid environment variable value");
    }

    let _lock = ENV_LOCK.lock();
    unsafe {
        std::env::set_var(name, value);
    }
    Ok(())
}

pub fn list() -> Vec<(String, String)> {
    let _lock = ENV_LOCK.lock();
    std::env::vars().collect()
}