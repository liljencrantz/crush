/**
Read login records from utmp/utmpx database.
 */
use std::fmt::{Display, Formatter};
use std::mem::transmute;
use std::str::Utf8Error;
use std::sync::Mutex;
use std::os::raw::c_short;
use chrono::{DateTime, Local, TimeZone};
use libc::{endutxent, getutxent, timeval};
use UtmpxType::{BootTime, DeadProcess, Empty, InitProcess, LoginProcess, NewTime, OldTime, ShutdownTime, UserProcess};

#[derive(Debug)]
pub struct Error {
    msg: String,
}

static MUTEX: Mutex<()> = Mutex::new(());

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.msg)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error {
            msg: e.to_string(),
        }
    }
}

impl From<Utf8Error> for Error {
    fn from(e: Utf8Error) -> Self {
        Error {
            msg: e.to_string(),
        }
    }
}

pub type LoginResult<T> = Result<T, Error>;

pub struct Login {
    pub tty: String,
    pub user: String,
    pub host: Option<String>,
    pub pid: i128,
    pub time: DateTime<Local>,
}

fn parse_string_record(s: &[i8]) -> LoginResult<String> {
    let mut res = String::with_capacity(s.len());
    for c in s {
        if *c == 0 {
            break;
        }
        res.push(unsafe { transmute::<i8, u8>(*c) as char })
    }
    Ok(res)
}

enum UtmpxType {
    Empty,
    BootTime,
    ShutdownTime,
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
    type Error = Error;

    fn try_from(value: c_short) -> Result<Self, Self::Error> {
        match value {
            libc::EMPTY => Ok(Empty),
            libc::BOOT_TIME => Ok(BootTime),
            libc::SHUTDOWN_TIME => Ok(ShutdownTime),
            libc::OLD_TIME => Ok(OldTime),
            libc::NEW_TIME => Ok(NewTime),
            libc::USER_PROCESS => Ok(UserProcess),
            libc::INIT_PROCESS => Ok(InitProcess),
            libc::LOGIN_PROCESS => Ok(LoginProcess),
            libc::DEAD_PROCESS => Ok(DeadProcess),
            _ => Err(Error { msg: "Invalid utmpx record type".to_string() })
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
        let host = parse_string_record(&record.ut_host)?;
        match UtmpxType::try_from(record.ut_type) {
            Ok(UserProcess) | Ok(InitProcess) | Ok(LoginProcess) =>
                res.push(Login {
                    tty: format!("/dev/{}", parse_string_record(&record.ut_line)?),
                    user: parse_string_record(&record.ut_user)?,
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
