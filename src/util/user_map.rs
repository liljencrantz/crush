use std::collections::HashMap;
use std::sync::Mutex;

use users::uid_t;
use users::User;

use lazy_static::lazy_static;

use crate::lang::{value::Value};

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
        Value::string(self.get(&uid).map(|u| u.name().to_str().unwrap_or("<illegal username>")).unwrap_or("<unknown user>"))
    }
}
