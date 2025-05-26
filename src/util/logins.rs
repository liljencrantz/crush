/**
Read login records from utmp/utmpx database.
 */
use std::sync::Mutex;
use std::os::raw::c_short;
use chrono::{DateTime, Local, TimeZone};
use nix::libc::{endutxent, getutxent, timeval};
use UtmpxType::{BootTime, DeadProcess, Empty, InitProcess, LoginProcess, NewTime, OldTime, UserProcess};
use crate::lang::errors::{CrushError, login_error};

static MUTEX: Mutex<()> = Mutex::new(());

pub type LoginResult<T> = Result<T, CrushError>;

pub struct Login {
    pub tty: String,
    pub user: String,
    pub host: Option<String>,
    pub pid: i128,
    pub time: DateTime<Local>,
}

trait ParseStringRecord {
    fn parse(&self) -> LoginResult<String>;
}

impl ParseStringRecord for [i8] {
    fn parse(&self) -> LoginResult<String> {
        let mut res = String::with_capacity(self.len());
        for c in self {
            if *c == 0 {
                break;
            }
            res.push(*c as u8 as char)
        }
        Ok(res)
    }
}

impl ParseStringRecord for [u8] {
    fn parse(&self) -> LoginResult<String> {
        let mut res = String::with_capacity(self.len());
        for c in self {
            if *c == 0 {
                break;
            }
            res.push(*c as char)
        }
        Ok(res)
    }
}

enum UtmpxType {
    Empty,
    BootTime,
    OldTime,
    NewTime,
    UserProcess,
    InitProcess,
    LoginProcess,
    DeadProcess,
}

fn parse_timeval(tv: &timeval) -> DateTime<Local> {
    Local.timestamp_nanos(tv.tv_usec as i64 * 1000 + (tv.tv_sec as i64) * 1000000000)
}

impl TryFrom<c_short> for UtmpxType {
    type Error = CrushError;

    fn try_from(value: c_short) -> Result<Self, Self::Error> {
        match value {
            nix::libc::EMPTY => Ok(Empty),
            nix::libc::BOOT_TIME => Ok(BootTime),
            nix::libc::OLD_TIME => Ok(OldTime),
            nix::libc::NEW_TIME => Ok(NewTime),
            nix::libc::USER_PROCESS => Ok(UserProcess),
            nix::libc::INIT_PROCESS => Ok(InitProcess),
            nix::libc::LOGIN_PROCESS => Ok(LoginProcess),
            nix::libc::DEAD_PROCESS => Ok(DeadProcess),
            _ => login_error("Invalid utmpx record type"),
        }
    }
}

pub fn list() -> LoginResult<Vec<Login>> {
    let mut res = Vec::new();
    let _lock = MUTEX.lock().unwrap();
    loop {
        let record_ptr = unsafe { getutxent() };
        if record_ptr.is_null() {
            break;
        }
        let record = unsafe { &*record_ptr };
        let host = record.ut_host.parse()?;
        match UtmpxType::try_from(record.ut_type) {
            Ok(UserProcess) | Ok(InitProcess) | Ok(LoginProcess) =>
                res.push(Login {
                    tty: format!("/dev/{}", record.ut_line.parse()?),
                    user: record.ut_user.parse()?,
                    time: parse_timeval(&record.ut_tv),
                    host: if host.is_empty() { None } else { Some(host) },
                    pid: record.ut_pid as i128,
                }),
            _ => {}
        }
    }
    unsafe { endutxent() };
    Ok(res)
}
