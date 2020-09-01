use std::collections::HashMap;
use std::sync::Mutex;

use lazy_static::lazy_static;

use crate::lang::value::Value;
use nix::unistd::{Uid, Gid, getuid};
use crate::lang::errors::{CrushResult, to_crush_error, error};
use std::ffi::CStr;

lazy_static! {
    static ref USER_MUTEX: Mutex<i32> = Mutex::new(0i32);
    static ref GROUP_MUTEX: Mutex<i32> = Mutex::new(0i32);
    static ref CURRENT_USERNAME: CrushResult<String> = {
        match create_user_map() {
            Ok(mut map) => {
                match map.remove(&getuid()) {
                    Some(v) => Ok(v),
                    None => error("Unknown user"),
                }
            },
            Err(e) => Err(e),
        }
    };
}

pub fn get_current_username() -> CrushResult<String> {
    CURRENT_USERNAME.as_ref().map(|s| s.clone()).map_err(|e| e.clone())
}

pub fn create_user_map() -> CrushResult<HashMap<Uid, String>> {
    let _user_lock = USER_MUTEX.lock().unwrap();
    let mut res = HashMap::new();
    unsafe {
        nix::libc::setpwent();
        loop {
            let passwd = nix::libc::getpwent();
            if passwd.is_null() {
                break;
            }
            res.insert(Uid::from_raw((*passwd).pw_uid), parse((*passwd).pw_name)?);
        }
        nix::libc::endpwent();
    }
    Ok(res)
}

pub fn create_group_map() -> CrushResult<HashMap<Gid, String>> {
    let _group_lock = GROUP_MUTEX.lock().unwrap();
    let mut res = HashMap::new();
    unsafe {
        nix::libc::setgrent();
        loop {
            let passwd = nix::libc::getgrent();
            if passwd.is_null() {
                break;
            }
            res.insert(Gid::from_raw((*passwd).gr_gid), parse((*passwd).gr_name)?);
        }
        nix::libc::endgrent();
    }
    Ok(res)
}

unsafe fn parse(s: *const i8) -> CrushResult<String> {
    Ok(to_crush_error(CStr::from_ptr(s).to_str())?.to_string())
}

pub fn get_uid(target_username: &str) -> CrushResult<Option<Uid>> {
    let _user_lock = USER_MUTEX.lock().unwrap();
    unsafe {
        nix::libc::setpwent();
        loop {
            let passwd = nix::libc::getpwent();
            if passwd.is_null() {
                break;
            }
            let pw_username = parse((*passwd).pw_name)?;
            if pw_username == target_username {
                nix::libc::endpwent();
                return Ok(Some(Uid::from_raw((*passwd).pw_uid)));
            }
        }
        nix::libc::endpwent();
    }
    Ok(None)
}

pub fn get_gid(target_groupname: &str) -> CrushResult<Option<Gid>> {
    let _group_lock = GROUP_MUTEX.lock().unwrap();
    unsafe {
        nix::libc::setgrent();
        loop {
            let grp = nix::libc::getgrent();
            if grp.is_null() {
                break;
            }
            let gr_groupname = parse((*grp).gr_name)?;
            if gr_groupname == target_groupname {
                nix::libc::endgrent();
                return Ok(Some(Gid::from_raw((*grp).gr_gid)));
            }
        }
        nix::libc::endgrent();
    }
    Ok(None)
}
