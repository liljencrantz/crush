use crate::lang::command::OutputType::Known;
use crate::lang::errors::{error, CrushResult};
use crate::lang::state::contexts::{CommandContext};
use crate::lang::data::table::ColumnType;
use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
use psutil::process::{Process, ProcessResult};
use signature::signature;

static FILE_OUTPUT_TYPE: [ColumnType; 3] = [
    ColumnType::new("fd", ValueType::Integer),
    ColumnType::new("path", ValueType::File),
    ColumnType::new("pid", ValueType::Integer),
];

#[signature(
    fs.fd.file,
    can_block = true,
    short = "Return a table stream containing information on all open files",
    output = Known(ValueType::table_input_stream(&FILE_OUTPUT_TYPE)),
    long = "fd:file accepts no arguments.")]
pub struct File {}

fn file(mut context: CommandContext) -> CrushResult<()> {
    File::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.initialize(&FILE_OUTPUT_TYPE)?;

    match psutil::process::processes() {
        Ok(procs) => {
            for proc in procs {
                for row in file_internal(proc)? {
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
                        Value::from(f.path),
                        Value::Integer(proc.pid() as i128),
                    ]));
                }
            }
        }
        Err(_) => {}
    }
    Ok(res)
}

#[cfg(target_os = "linux")]
pub mod procfs {
    use crate::lang::state::scope::Scope;
    use crate::util::user_map::create_user_map;
    use nix::unistd::{Uid};
    use std::collections::HashMap;
    use termion::input::TermRead;
    use crate::util::hex::from_hex;
    use dns_lookup::lookup_addr;
    use std::collections::hash_map::Entry;
    use crate::lang::pipe::OutputStream;
    use std::net::Ipv6Addr;
    use crate::lang::printer::Printer;
    use std::path::PathBuf;
    use super::*;

    static NET_OUTPUT_TYPE: [ColumnType; 9] =[
        ColumnType::new("type", ValueType::String),
        ColumnType::new("local_ip", ValueType::String),
        ColumnType::new("local_port", ValueType::Integer),
        ColumnType::new("remote_host", ValueType::String),
        ColumnType::new("remote_ip", ValueType::String),
        ColumnType::new("remote_port", ValueType::Integer),
        ColumnType::new("inode", ValueType::Integer),
        ColumnType::new("creator", ValueType::String),
        ColumnType::new("pid", ValueType::Any),
    ];

    static UNIX_OUTPUT_TYPE: [ColumnType; 3] = [
        ColumnType::new("inode", ValueType::Integer),
        ColumnType::new("path", ValueType::Any),
        ColumnType::new("pid", ValueType::Any),
    ];

    #[signature(
        network,
        can_block = true,
        short = "Return a table stream containing information on all open network sockets",
        output = Known(ValueType::table_input_stream(&NET_OUTPUT_TYPE)),
        long = "fd:network accepts no arguments.")]
    pub struct Network {}

    fn parse_addr(addr: &str) -> CrushResult<(String, u16)> {
        let parts = addr.split(':').collect::<Vec<_>>();
        if parts.len() != 2 {
            return error("Invalid address");
        }
        let port_bytes = from_hex(parts[1])?;
        let port = (port_bytes[0] as u16) << 8 | port_bytes[1] as u16;

        let ip = match parts[0].len() {
            8 => {
                let ip_bytes = from_hex(parts[0])?;
                format!(
                    "{}.{}.{}.{}",
                    ip_bytes[3], ip_bytes[2], ip_bytes[1], ip_bytes[0])
            }
            32 => {
                let obtuse = format!(
                    "{}:{}:{}:{}:{}:{}:{}:{}",
                    &parts[0][0..4],
                    &parts[0][4..8],
                    &parts[0][8..12],
                    &parts[0][12..16],
                    &parts[0][16..20],
                    &parts[0][20..24],
                    &parts[0][24..28],
                    &parts[0][28..32],
                );
                let ip = obtuse.parse::<Ipv6Addr>()?;
                ip.to_string()
            }
            _ => return error(format!("Invalid ip address {}", parts[0])),
        };

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

    fn extract_sockets(proc: ProcessResult<Process>, pids: &mut HashMap<u32, Vec<u32>>) -> ProcessResult<()> {
        let proc = proc?;
        match proc.open_files() {
            Ok(files) => {
                for f in files {
                    match f.path.to_str() {
                        Some(s) => {
                            if s.starts_with("socket:[") && s.ends_with("]") {
                                let inode = s.strip_prefix("socket:[").unwrap().strip_suffix("]").unwrap()
                                    .parse::<u32>()?;
                                match pids.entry(inode) {
                                    Entry::Occupied(mut e) => {
                                        e.get_mut().push(proc.pid());
                                    }
                                    Entry::Vacant(e) => {
                                        e.insert(vec![proc.pid()]);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Err(_) => {}
        }
        Ok(())
    }

    fn network(context: CommandContext) -> CrushResult<()> {
        Network::parse(context.arguments.clone(), &context.global_state.printer())?;
        let users = create_user_map()?;
        let mut hosts = HashMap::new();
        let output = context.output.initialize(&NET_OUTPUT_TYPE)?;

        let mut pids = HashMap::new();

        match psutil::process::processes() {
            Ok(procs) => {
                for proc in procs {
                    extract_sockets(proc, &mut pids)?;
                }
            }
            Err(_) => return error("Failed to list processes"),
        }

        handle_socket_file(&users, &mut pids, &mut hosts, "tcp", &context.global_state.printer(), &output)?;
        handle_socket_file(&users, &mut pids, &mut hosts, "udp", &context.global_state.printer(), &output)?;
        handle_socket_file(&users, &mut pids, &mut hosts, "tcp6", &context.global_state.printer(), &output)?;
        handle_socket_file(&users, &mut pids, &mut hosts, "udp6", &context.global_state.printer(), &output)?;

        Ok(())
    }

    fn handle_socket_file(
        users: &HashMap<Uid, String>,
        pids: &mut HashMap<u32, Vec<u32>>,
        hosts: &mut HashMap<String, String>,
        file_type: &str,
        printer: &Printer,
        output: &OutputStream) -> CrushResult<()> {
        let mut f = std::fs::File::open(&format!("/proc/net/{}", file_type))?;
        // Skip header
        f.read_line()?;

        while let Some(line) = f.read_line()? {
            let trimmed = line.trim_start_matches(' ').trim_end_matches(' ');
            let parts = trimmed.split(' ').filter(|s| !s.is_empty()).collect::<Vec<_>>();
            if parts.len() == 0 {
                break;
            }
            if parts.len() < 10 {
                printer.error(&format!("Invalid data in /proc/net/{}:\n{}", file_type, &line));
                continue;
            }

            let uid = parts[7].parse::<u32>()?;

            let (local_ip, local_port) = parse_addr(parts[1])?;
            let (remote_ip, remote_port) = parse_addr(parts[2])?;
            let inode = parts[9].parse::<u32>()?;

            match pids.entry(inode) {
                Entry::Occupied(e) => {
                    for pid in e.get().iter() {
                        output.send(Row::new(vec![
                            Value::from(file_type),
                            Value::from(&local_ip),
                            Value::Integer(local_port as i128),
                            Value::from(lookup(&remote_ip, hosts)?),
                            Value::from(&remote_ip),
                            Value::Integer(remote_port as i128),
                            Value::Integer(inode as i128),
                            users.get(&Uid::from_raw(uid)).map(|s| Value::from(s)).unwrap_or_else(|| Value::from("?")),
                            Value::Integer(*pid as i128),
                        ]))?;
                    }
                }
                Entry::Vacant(_) => {
                    output.send(Row::new(vec![
                        Value::from(file_type),
                        Value::from(local_ip),
                        Value::Integer(local_port as i128),
                        Value::from(lookup(&remote_ip, hosts)?),
                        Value::from(remote_ip),
                        Value::Integer(remote_port as i128),
                        Value::Integer(inode as i128),
                        users.get(&nix::unistd::Uid::from_raw(uid)).map(|s| Value::from(s)).unwrap_or_else(|| Value::from("?")),
                        Value::Empty,
                    ]))?;
                }
            }
        }
        Ok(())
    }

    #[signature(
        unix,
        can_block = true,
        short = "Return a table stream containing information on all open unix sockets",
        output = Known(ValueType::table_input_stream(&UNIX_OUTPUT_TYPE)),
        long = "fd:unix accepts no arguments.")]
    pub struct Unix {}

    fn unix(context: CommandContext) -> CrushResult<()> {
        Unix::parse(context.arguments.clone(), &context.global_state.printer())?;
        let output = context.output.initialize(&UNIX_OUTPUT_TYPE)?;

        let mut pids = HashMap::new();

        match psutil::process::processes() {
            Ok(procs) => {
                for proc in procs {
                    extract_sockets(proc, &mut pids)?;
                }
            }
            Err(_) => return error("Failed to list processes"),
        }

        let mut f = std::fs::File::open("/proc/net/unix")?;
        // Skip header
        f.read_line()?;

        while let Some(line) = f.read_line()? {
            let trimmed = line.trim_start_matches(' ').trim_end_matches(' ');
            let parts = trimmed.split(' ').filter(|s| !s.is_empty()).collect::<Vec<_>>();
            if parts.len() == 0 {
                break;
            }
            if parts.len() < 7 {
                context.global_state.printer().error(&format!("Invalid data in /proc/net/unix:\n{}", &line));
                continue;
            }

            let inode = parts[6].parse::<u32>()?;

            let path = if parts.len() >= 8 {
                Value::from(PathBuf::from(parts[7]))
            } else {
                Value::Empty
            };

            match pids.entry(inode) {
                Entry::Occupied(e) => {
                    for pid in e.get().iter() {
                        output.send(Row::new(vec![
                            Value::Integer(inode as i128),
                            path.clone(),
                            Value::Integer(*pid as i128),
                        ]))?;
                    }
                }
                Entry::Vacant(_) => {
                    output.send(Row::new(vec![
                        Value::Integer(inode as i128),
                        path,
                        Value::Empty,
                    ]))?;
                }
            }
        }

        Ok(())
    }
}
