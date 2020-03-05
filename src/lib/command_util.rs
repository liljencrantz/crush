use std::collections::HashMap;
use std::sync::Mutex;

use users::uid_t;
use users::User;

use lazy_static::lazy_static;

use crate::lang::{value::Value, table::ColumnType};
use crate::errors::{CrushError, CrushResult, error, argument_error};

pub fn find_field_from_str(needle: &str, haystack: &Vec<ColumnType>) -> CrushResult<usize> {
    for (idx, field) in haystack.iter().enumerate() {
        if field.name.as_ref().map(|v| v.as_ref().eq(needle)).unwrap_or(false) {
            return Ok(idx);
        }
    }

    argument_error(format!(
                "Unknown column {}, available columns are {}",
                needle,
                haystack.iter().map(|t| t.val_or_empty().to_string()).collect::<Vec<String>>().join(", "),
            ).as_str())
}

pub fn find_field(needle_vec: &Vec<Box<str>>, haystack: &Vec<ColumnType>) -> CrushResult<usize> {
    if needle_vec.len() != 1 {
        argument_error("Expected direct field")
    } else {
        let needle = needle_vec[0].as_ref();
        for (idx, field) in haystack.iter().enumerate() {
            if field.name.as_ref().map(|v| v.as_ref().eq(needle)).unwrap_or(false) {
                return Ok(idx);
            }
        }

        error(format!(
            "Unknown column {}, available columns are {}",
            needle,
            haystack.iter().map(|t| t.val_or_empty().to_string()).collect::<Vec<String>>().join(", "),
        ).as_str())
    }
}

lazy_static! {
    static ref USER_MUTEX: Mutex<i32> = Mutex::new(0i32);
}

pub fn create_user_map() -> HashMap<uid_t, User> {
    let _user_lock = USER_MUTEX.lock().unwrap();

    let mut h: HashMap<uid_t, users::User> = HashMap::new();
    let iter = unsafe {users::all_users()};
    for user in iter {
        h.insert(user.uid(), user);
    }
    h
}

pub trait UserMap {
    fn get_name(&self, uid: uid_t) -> Value;
}

impl UserMap for HashMap<uid_t, User> {
    fn get_name(&self, uid: uid_t) -> Value {
        Value::text(self.get(&uid).map(|u| u.name().to_str().unwrap_or("<illegal username>")).unwrap_or("<unknown user>"))
    }
}
