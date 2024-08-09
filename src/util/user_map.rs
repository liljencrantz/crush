use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::os::raw::c_char;

use nix::unistd::{Uid, Gid, getuid};
use crate::lang::errors::{CrushResult, data_error, error};
use std::ffi::CStr;
use libc::gid_t;
use std::path::PathBuf;
use libc::{passwd, uid_t};
use crate::argument_error_legacy;

static USER_MUTEX: Mutex<i32> = Mutex::new(0i32);
static GROUP_MUTEX: Mutex<i32> = Mutex::new(0i32);

pub fn get_current_username() -> CrushResult<&'static str> {
    static CELL: OnceLock<CrushResult<String>> = OnceLock::new();
    let cu = CELL.get_or_init(|| {
        match create_user_map() {
            Ok(mut map) => {
                match map.remove(&getuid()) {
                    Some(v) => Ok(v),
                    None => error("Unknown user"),
                }
            },
            Err(e) => Err(e),
        }
    });
    match cu {
        Ok(s) => Ok(s.as_str()),
        Err(e) => data_error(e.message()),
    }
}

pub fn create_user_map() -> CrushResult<HashMap<Uid, String>> {
    let _user_lock = USER_MUTEX.lock().unwrap();
    let mut res = HashMap::new();
    unsafe {
        libc::setpwent();
        loop {
            let passwd = libc::getpwent();
            if passwd.is_null() {
                break;
            }
            res.insert(Uid::from_raw((*passwd).pw_uid), parse((*passwd).pw_name)?);
        }
        libc::endpwent();
    }
    Ok(res)
}

pub struct UserData {
    pub name: String,
    pub home: PathBuf,
    pub shell: PathBuf,
    pub information: String,
    pub uid: uid_t,
    pub gid: gid_t,
}

pub fn get_all_users() -> CrushResult<Vec<UserData>> {
    let _user_lock = USER_MUTEX.lock().unwrap();
    let mut res = Vec::new();
    unsafe {
        libc::setpwent();
        loop {
            let passwd = nix::libc::getpwent();
            if passwd.is_null() {
                break;
            }
            match UserData::new(&*passwd) {
                Ok(d) => res.push(d),
                Err(e) => return Err(e),
            }
        }
        libc::endpwent();
    }
    Ok(res)
}

impl UserData {
    unsafe fn new(data: &passwd) -> CrushResult<UserData> {
        Ok(UserData {
            name: parse(data.pw_name)?,
            home: PathBuf::from(parse(data.pw_dir)?),
            shell: PathBuf::from(parse(data.pw_shell)?),
            information: parse(data.pw_gecos)?,
            uid: data.pw_uid,
            gid: data.pw_gid,
        })
    }
}

pub fn get_user(input_name: &str) -> CrushResult<UserData> {
    let _user_lock = USER_MUTEX.lock().unwrap();
    unsafe {
        libc::setpwent();
        loop {
            let passwd = libc::getpwent();
            if passwd.is_null() {
                return argument_error_legacy(format!("Unknown user {}", input_name));
            }
            let name = parse((*passwd).pw_name)?;
            if name == input_name {
                let res = UserData::new(&*passwd);
                libc::endpwent();
                return res;
            }
        }
    }
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

unsafe fn parse(s: *const c_char) -> CrushResult<String> {
    Ok(CStr::from_ptr(s).to_str()?.to_string())
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
