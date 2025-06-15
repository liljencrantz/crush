use std::collections::HashMap;
use std::os::raw::c_char;
use std::sync::{Mutex, OnceLock};

use crate::lang::errors::{CrushResult, data_error, error};
use nix::libc::passwd;
use nix::unistd::getuid;
use std::ffi::CStr;
use std::path::PathBuf;

static USER_MUTEX: Mutex<i32> = Mutex::new(0i32);

pub fn get_current_username() -> CrushResult<&'static str> {
    static CELL: OnceLock<CrushResult<String>> = OnceLock::new();
    let cu = CELL.get_or_init(|| match create_user_map() {
        Ok(mut map) => match map.remove(&sysinfo::Uid::try_from(getuid().as_raw() as usize)?) {
            Some(v) => Ok(v),
            None => error("Unknown user"),
        },
        Err(e) => Err(e),
    });
    match cu {
        Ok(s) => Ok(s.as_str()),
        Err(e) => data_error(e.message()),
    }
}

pub fn create_user_map() -> CrushResult<HashMap<sysinfo::Uid, String>> {
    let mut res = HashMap::new();
    for u in sysinfo::Users::new_with_refreshed_list().list() {
        res.insert(u.id().clone(), u.name().to_string());
    }
    Ok(res)
}

pub struct UserData {
    pub name: String,
    pub home: PathBuf,
    pub shell: PathBuf,
    pub information: String,
    pub uid: sysinfo::Uid,
    pub gid: sysinfo::Gid,
}

pub fn get_all_users() -> CrushResult<Vec<UserData>> {
    let _user_lock = USER_MUTEX.lock().unwrap();
    let mut res = Vec::new();
    unsafe {
        nix::libc::setpwent();
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
        nix::libc::endpwent();
    }
    Ok(res)
}

impl UserData {
    unsafe fn new(data: &passwd) -> CrushResult<UserData> {
        unsafe {
            Ok(UserData {
                name: cstring_tostring(data.pw_name)?,
                home: PathBuf::from(cstring_tostring(data.pw_dir)?),
                shell: PathBuf::from(cstring_tostring(data.pw_shell)?),
                information: cstring_tostring(data.pw_gecos)?,
                uid: sysinfo::Uid::try_from(data.pw_uid as usize)?,
                gid: sysinfo::Gid::try_from(data.pw_gid as usize)?,
            })
        }
    }
}

pub fn get_user(input_name: &str) -> CrushResult<UserData> {
    let all = get_all_users()?;
    Ok(all
        .into_iter()
        .find(|u| u.name == input_name)
        .ok_or(format!("Unknown user {}", input_name))?)
}

pub fn create_group_map() -> CrushResult<HashMap<sysinfo::Gid, String>> {
    let mut res = HashMap::new();
    for g in sysinfo::Groups::new_with_refreshed_list().list() {
        res.insert(*g.id(), g.name().to_string());
    }
    Ok(res)
}

unsafe fn cstring_tostring(s: *const c_char) -> CrushResult<String> {
    unsafe { Ok(CStr::from_ptr(s).to_str()?.to_string()) }
}

pub fn get_uid(target_username: &str) -> CrushResult<Option<sysinfo::Uid>> {
    for u in sysinfo::Users::new_with_refreshed_list().list() {
        if u.name() == target_username {
            return Ok(Some(u.id().clone()));
        }
    }
    Ok(None)
}

pub fn get_gid(target_groupname: &str) -> CrushResult<Option<sysinfo::Gid>> {
    for g in sysinfo::Groups::new_with_refreshed_list().list() {
        if g.name() == target_groupname {
            return Ok(Some(*g.id()));
        }
    }
    Ok(None)
}
