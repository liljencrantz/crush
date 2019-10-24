use crate::data::{CellFnurp, Cell};
use crate::errors::{JobResult, JobError};
use std::sync::Mutex;
use lazy_static::lazy_static;
use std::collections::HashMap;
use users::uid_t;
use users::User;

pub fn find_field(needle: &str, haystack: &Vec<CellFnurp>) -> JobResult<usize> {
    for (idx, field) in haystack.iter().enumerate() {
        if field.name.as_ref().map(|v| v.as_ref().eq(needle)).unwrap_or(false) {
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

lazy_static! {
    static ref user_mutex: Mutex<i32> = Mutex::new(0i32);
}

pub fn create_user_map() -> HashMap<uid_t, User> {
    let user_lock = user_mutex.lock().unwrap();

    let mut h: HashMap<uid_t, users::User> = HashMap::new();
    let iter = unsafe {users::all_users()};
    for user in iter {
        h.insert(user.uid(), user);
    }
    h
}

pub trait UserMap {
    fn get_name(&self, uid: uid_t) -> Cell;
}

impl UserMap for HashMap<uid_t, User> {
    fn get_name(&self, uid: uid_t) -> Cell {
        Cell::text(self.get(&uid).map(|u| u.name().to_str().unwrap_or("<illegal username>")).unwrap_or("<unknown user>"))
    }
}
