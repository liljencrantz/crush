use crate::lang::argument::ArgumentHandler;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{error, to_crush_error, CrushResult, data_error};
use crate::lang::execution_context::{ArgumentVector, CommandContext};
use crate::lang::data::scope::Scope;
use crate::lang::data::table::ColumnType;
use crate::util::user_map::create_user_map;
use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
use chrono::Duration;
use lazy_static::lazy_static;
use nix::sys::signal;
use nix::unistd::{Pid, Uid};
use psutil::process::os::unix::ProcessExt;
use psutil::process::{Process, ProcessResult, Status, ProcessError, OpenFile};
use signature::signature;
use std::collections::HashMap;
use std::str::FromStr;
use termion::input::TermRead;
use crate::util::hex::from_hex;
use dns_lookup::lookup_addr;
use std::convert::TryFrom;
use std::collections::hash_map::Entry;
use std::io::Read;

lazy_static! {
    static ref FILE_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("fd", ValueType::Integer),
        ColumnType::new("path", ValueType::File),
        ColumnType::new("pid", ValueType::Integer),
    ];
    static ref NET_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("type", ValueType::String),
        ColumnType::new("local_ip", ValueType::String),
        ColumnType::new("local_port", ValueType::Integer),
        ColumnType::new("remote_host", ValueType::String),
        ColumnType::new("remote_ip", ValueType::String),
        ColumnType::new("remote_port", ValueType::Integer),
        ColumnType::new("user", ValueType::String),
    ];
}

#[signature(
file,
can_block = true,
short = "Return a table stream containing information on all open files",
output = Known(ValueType::TableStream(FILE_OUTPUT_TYPE.clone())),
long = "fd:file accepts no arguments.")]
pub struct File {}

fn file(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let output = context.output.initialize(FILE_OUTPUT_TYPE.clone())?;

    match psutil::process::processes() {
        Ok(procs) => {
            for proc in procs {
                for row in to_crush_error(file_internal(proc))? {
                    output.send(row)?;
                }
            }
        }
        Err(_) => return error("Failed to list processes"),
    }
    Ok(())
}

fn file_internal(proc: ProcessResult<Process>) -> ProcessResult<Vec<Row>> {
    let proc = proc?;
    let mut res = Vec::new();
    match proc.open_files() {
        Ok(files) => {
            for f in files {
                if f.path.starts_with("/") {
                    res.push(Row::new(vec![
                        Value::Integer(f.fd.unwrap_or(0) as i128),
                        Value::File(f.path),
                        Value::Integer(proc.pid() as i128),
                    ]));
                }
            }
        }
        Err(_) => {}
    }
    Ok(res)
}

#[signature(
net,
can_block = true,
short = "Return a table stream containing information on all open network sockets",
output = Known(ValueType::TableStream(NET_OUTPUT_TYPE.clone())),
long = "fd:net accepts no arguments.")]
pub struct Net {}

fn parse_addr(addr: &str) -> CrushResult<(String, u16)> {
    let parts = addr.split(':').collect::<Vec<_>>();
    if parts.len() != 2 {
        return data_error("Invalid address");
    }
    let ip_bytes = from_hex(parts[0])?;
    let port_bytes = from_hex(parts[1])?;
    let port = (port_bytes[0] as u16) << 8 | port_bytes[1] as u16;

    let ip = format!(
        "{}.{}.{}.{}",
        ip_bytes[3], ip_bytes[2], ip_bytes[1], ip_bytes[0]);

    Ok((ip, port))
}

fn lookup(ip: &str, cache: &mut HashMap<String, String>) -> CrushResult<String> {
    match cache.entry(ip.to_string()) {
        Entry::Occupied(e) => {
            Ok(e.get().clone())
        }
        Entry::Vacant(e) => {
            let ip: std::net::IpAddr = ip.parse().unwrap();
            let host = lookup_addr(&ip).unwrap_or_else(|_| "?".to_string());
            e.insert(host.clone());
            Ok(host)
        }
    }
}

fn net(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    let users = create_user_map()?;
    let mut hosts = HashMap::new();
    let output = context.output.initialize(NET_OUTPUT_TYPE.clone())?;

    let mut f = to_crush_error(std::fs::File::open("/proc/net/tcp"))?;

    // Skip header
    to_crush_error(f.read_line())?;

    while let Some(line) = to_crush_error(f.read_line())? {
        let trimmed = line.trim_start_matches(' ').trim_end_matches(' ');
        let parts = trimmed.split(' ').filter(|s| !s.is_empty()).collect::<Vec<_>>();
        if parts.len() == 0 {
            break;
        }
        if parts.len() < 13 {
            return data_error(format!("Invalid data in /proc/net/tcp:\n{}", &line));
        }

        let uid = to_crush_error(parts[7].parse::<u32>())?;

        let (local_ip, local_port) = parse_addr(parts[1])?;
        let (remote_ip, remote_port) = parse_addr(parts[2])?;

        output.send(Row::new(vec![
            Value::string("tcp"),
            Value::String(local_ip),
            Value::Integer(local_port as i128),
            Value::String(lookup(&remote_ip, &mut hosts)?),
            Value::String(remote_ip),
            Value::Integer(remote_port as i128),
            users.get(&nix::unistd::Uid::from_raw(uid)).map(|s| Value::string(s)).unwrap_or_else(|| Value::string("?")),
        ]))?;
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "fd",
        Box::new(move |fd| {
            File::declare(fd)?;
            Net::declare(fd)?;
            Ok(())
        }),
    )?;
    Ok(())
}
