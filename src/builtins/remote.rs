use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{error, mandate, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::signature::files::Files;
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::scope::Scope;
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::data::table::{ColumnFormat, ColumnType, Row};
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::file::home;
use crossbeam::channel::unbounded;
use lazy_static::lazy_static;
use signature::signature;
use ssh2::KnownHostFileKind;
use ssh2::{CheckResult, HostKeyType, KnownHostKeyFormat, Session};
use std::cmp::min;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use crate::util::user_map::get_current_username;
use crate::lang::completion::Completion;
use crate::lang::completion::parse::{PartialCommandResult, LastArgument};
use std::convert::TryFrom;
use std::sync::OnceLock;
use crate::util::escape::{escape, escape_without_quotes};

pub fn identity_output_type() -> &'static Vec<ColumnType> {
    static CELL: OnceLock<Vec<ColumnType>> = OnceLock::new();
    CELL.get_or_init(|| vec![
        ColumnType::new("identity", ValueType::String),
        ColumnType::new("public_key", ValueType::Binary),
    ])
}
pub fn host_output_type() -> &'static Vec<ColumnType> {
    static CELL: OnceLock<Vec<ColumnType>> = OnceLock::new();
    CELL.get_or_init(|| vec![
        ColumnType::new("host", ValueType::String),
        ColumnType::new("public_key", ValueType::String),
    ])
}

fn parse(
    mut host: String,
    default_username: &Option<String>,
) -> CrushResult<(String, String, u16)> {
    let username;
    if host.contains('@') {
        let mut tmp = host.splitn(2, '@');
        username = tmp.next().unwrap().to_string();
        host = tmp.next().unwrap().to_string();
    } else {
        username = default_username.clone()
            .unwrap_or(get_current_username()?.to_string());
    }

    let port: u16;
    if !host.contains(':') {
        port = 22;
    } else {
        let mut parts = host.split(':');
        let tmp = parts.next().unwrap().to_string();
        port = parts.next().unwrap().parse::<u16>()?;
        drop(parts);
        host = tmp;
    }
    Ok((host, username, port))
}

fn run_remote(
    cmd: &Vec<u8>,
    env: &Scope,
    host: String,
    default_username: &Option<String>,
    password: &Option<String>,
    host_file: &PathBuf,
    ignore_host_file: bool,
    allow_not_found: bool,
) -> CrushResult<Value> {
    let (host, username, port) = parse(host, &default_username)?;

    let tcp = TcpStream::connect(&format!("{}:{}", host, port))?;
    let mut sess = Session::new()?;

    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    if !ignore_host_file {
        let mut known_hosts = sess.known_hosts()?;
        known_hosts.read_file(host_file, KnownHostFileKind::OpenSSH)?;
        let (key, key_type) = mandate(
            sess.host_key(),
            &format!("Could not fetch host key for {}", host),
        )?;
        match known_hosts.check_port(&host, port, key) {
            CheckResult::Match => {}
            CheckResult::Mismatch => return error("Host mismatch"),
            CheckResult::NotFound => {
                if !allow_not_found {
                    return error(&format!("Host {} missing from known host file", host));
                } else {
                    let key_format = match key_type {
                        HostKeyType::Unknown => KnownHostKeyFormat::Unknown,
                        HostKeyType::Rsa => KnownHostKeyFormat::SshRsa,
                        HostKeyType::Dss => KnownHostKeyFormat::SshDss,
                        HostKeyType::Ecdsa256 => KnownHostKeyFormat::Ecdsa256,
                        HostKeyType::Ecdsa384 => KnownHostKeyFormat::Ecdsa384,
                        HostKeyType::Ecdsa521 => KnownHostKeyFormat::Ecdsa521,
                        HostKeyType::Ed255219 => KnownHostKeyFormat::Ed255219,
                    };
                    known_hosts.add(&host, key, "Added by Crush", key_format)?;
                    known_hosts.write_file(host_file, KnownHostFileKind::OpenSSH)?;
                }
            }
            CheckResult::Failure => return error("Host validation check failure"),
        }
    }

    if let Some(pass) = password {
        sess.userauth_password(&username, pass)?
    } else {
        sess.userauth_agent(&username)?;
    }

    let mut channel = sess.channel_session()?;
    channel.exec("crush --pup")?;
    channel.write(cmd)?;
    channel.send_eof()?;
    let mut out_buf = Vec::new();
    channel.read_to_end(&mut out_buf)?;
    let res = deserialize(&out_buf, env)?;
    channel.wait_close()?;
    Ok(res)
}

fn ssh_host_complete(
    cmd: &PartialCommandResult,
    _cursor: usize,
    _scope: &Scope,
    res: &mut Vec<Completion>,
) -> CrushResult<()> {
    let session = Session::new()?;
    let mut known_hosts = session.known_hosts()?;
    let host_file = home()?.join(".ssh/known_hosts");

    known_hosts.read_file(&host_file, KnownHostFileKind::OpenSSH)?;
    for host in known_hosts.iter()? {
        match &cmd.last_argument {
            LastArgument::Unknown => {
                let completion = escape(host.name().unwrap_or(""));
                res.push(Completion::new(
                    completion,
                    host.name().unwrap_or(""),
                    0,
                ))
            }

            LastArgument::QuotedString(stripped_prefix) => {
                let completion = host.name().unwrap_or("");
                if completion.starts_with(stripped_prefix) && completion.len() > 0 {
                    res.push(Completion::new(
                        format!("{}\" ", escape_without_quotes(&completion[stripped_prefix.len()..])),
                        host.name().unwrap_or(""),
                        0,
                    ));
                }
            }

            _ => {}
        }
    }
    Ok(())
}


#[signature(
    remote.exec,
    can_block = true,
    short = "Execute a command on a remote host",
    long = "    Execute the specified command on the soecified host"
)]
struct Exec {
    #[description("the command to execute.")]
    command: Command,
    #[custom_completion(ssh_host_complete)]
    #[description("host to execute the command on.")]
    host: String,
    #[description("username on remote machines.")]
    username: Option<String>,
    #[description("password on remote machines. If no password is provided, agent authentication will be used."
    )]
    password: Option<String>,
    #[description("(~/.ssh/known_hosts) known hosts file.")]
    host_file: Files,
    #[description("skip checking the know hosts file.")]
    #[default(false)]
    ignore_host_file: bool,
    #[description("allow missing hosts in the known hosts file. Missing hosts will be automatically added to the file."
    )]
    #[default(false)]
    allow_not_found: bool,
}

fn exec(context: CommandContext) -> CrushResult<()> {
    let cfg: Exec = Exec::parse(context.arguments, &context.global_state.printer())?;
    let host_file = if cfg.host_file.had_entries() {
        PathBuf::try_from(cfg.host_file)?
    } else {
        home()?.join(".ssh/known_hosts")
    };
    let mut in_buf = Vec::new();
    serialize(&Value::Command(cfg.command), &mut in_buf)?;
    context.output.send(run_remote(
        &in_buf,
        &context.scope,
        cfg.host,
        &cfg.username,
        &cfg.password,
        &host_file,
        cfg.ignore_host_file,
        cfg.allow_not_found,
    )?)
}

#[signature(
    remote.pexec,
    can_block = true,
    short = "Execute a command on a set of hosts",
    long = "    Execute the specified command all specified hosts",
    output = Known(ValueType::TableInputStream(pexec_output_type().clone())),
)]
struct Pexec {
    #[description("the command to execute.")]
    #[description("the command to execute.")]
    command: Command,
    #[unnamed()]
    #[custom_completion(ssh_host_complete)]
    #[description("hosts to execute the command on.")]
    host: Vec<String>,
    #[description("maximum number of hosts to run on in parallel.")]
    #[default(32)]
    parallel: i128,
    #[description("username on remote machines.")]
    username: Option<String>,
    #[description("password on remote machines. If no password is provided, agent authentication will be used."
    )]
    password: Option<String>,
    #[description("(~/.ssh/known_hosts) known hosts file.")]
    host_file: Files,
    #[description("skip checking the know hosts file.")]
    #[default(false)]
    ignore_host_file: bool,
    #[description("allow missing hosts in the known hosts file. Missing hosts will be automatically added to the file."
    )]
    #[default(false)]
    allow_not_found: bool,
}

pub fn pexec_output_type() -> &'static Vec<ColumnType> {
    static CELL: OnceLock<Vec<ColumnType>> = OnceLock::new();
    CELL.get_or_init(|| vec![
        ColumnType::new("host", ValueType::String),
        ColumnType::new("result", ValueType::Any),
    ])
}

fn pexec(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Pexec = Pexec::parse(context.remove_arguments(), &context.global_state.printer())?;
    let host_file = if cfg.host_file.had_entries() {
        PathBuf::try_from(cfg.host_file)?
    } else {
        home()?.join(".ssh/known_hosts")
    };

    let (host_send, host_recv) = unbounded::<String>();
    let (result_send, result_recv) = unbounded::<(String, Value)>();

    let mut in_buf = Vec::new();

    serialize(&Value::Command(cfg.command), &mut in_buf)?;

    for host in &cfg.host {
        host_send.send(host.clone())?;
    }

    drop(host_send);

    let thread_count = min(cfg.parallel as usize, cfg.host.len());
    for _ in 0..thread_count {
        let my_recv = host_recv.clone();
        let my_send = result_send.clone();
        let my_buf = in_buf.clone();
        let my_env = context.scope.clone();
        let my_username = cfg.username.clone();
        let my_password = cfg.password.clone();
        let my_host_file = host_file.clone();
        let my_ignore_host_file = cfg.ignore_host_file;
        let my_allow_not_found = cfg.allow_not_found;

        context.spawn(
            "remote:pexec",
            move || {
                while let Ok(host) = my_recv.recv() {
                    let res = run_remote(
                        &my_buf,
                        &my_env,
                        host.clone(),
                        &my_username,
                        &my_password,
                        &my_host_file,
                        my_ignore_host_file,
                        my_allow_not_found,
                    )?;
                    my_send.send((host, res))?;
                }
                Ok(())
            })?;
    }

    drop(result_send);
    let output = context.output.initialize(pexec_output_type())?;

    while let Ok((host, val)) = result_recv.recv() {
        output.send(Row::new(vec![Value::from(host), val]))?;
    }

    Ok(())
}

#[signature(
    remote.identity,
    can_block = true,
    output = Known(ValueType::TableInputStream(identity_output_type().clone())),
    short = "List all known ssh-agent identities"
)]
struct Identity {}

fn identity(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(identity_output_type())?;
    let sess = Session::new()?;
    let mut agent = sess.agent()?;

    agent.connect()?;
    agent.list_identities()?;

    for identity in agent.identities()? {
        output.send(Row::new(vec![
            Value::from(identity.comment().to_string()),
            Value::from(identity.blob()),
        ]))?;
    }
    Ok(())
}

mod host {
    use super::*;
    use std::convert::TryInto;

    #[signature(
        remote.host.list,
        can_block = true,
        output = super::Known(ValueType::TableInputStream(host_output_type().clone())),
        short = "List all known hosts",
        long = "If a given host key has no hostname, the hostname will be the empty string"
    )]
    pub struct List {
        #[description("(~/.ssh/known_hosts) known hosts file.")]
        host_file: Files,
    }

    fn list(context: CommandContext) -> CrushResult<()> {
        let cfg: List = List::parse(context.arguments, &context.global_state.printer())?;
        let output = context
            .output
            .initialize(host_output_type())?;
        let session = Session::new()?;

        let mut known_hosts = session.known_hosts()?;

        // Initialize the known hosts with a global known hosts file
        let host_file = if cfg.host_file.had_entries() {
            PathBuf::try_from(cfg.host_file)?
        } else {
            home()?.join(".ssh/known_hosts")
        };
        known_hosts.read_file(&host_file, KnownHostFileKind::OpenSSH)?;
        for host in known_hosts.iter()? {
            output.send(Row::new(vec![
                Value::from(host.name().unwrap_or("")),
                Value::from(host.key()),
            ]))?;
        }
        Ok(())
    }

    #[signature(
        remote.host.remove,
        can_block = true,
        short = "Remove hosts from known_hosts file",
        output = Known(ValueType::Integer),
        long = "Remove all hosts that match both the host and the key filters.\n    Returns the number of host entries deleted."
    )]
    pub struct Remove {
        #[description("(~/.ssh/known_hosts) known hosts file.")]
        host_file: Files,
        #[description("host filter.")]
        host: Patterns,
        #[description("key filter.")]
        key: Patterns,
    }

    fn remove(context: CommandContext) -> CrushResult<()> {
        let cfg: Remove = Remove::parse(context.arguments, &context.global_state.printer())?;
        let host_file = if cfg.host_file.had_entries() {
            cfg.host_file.clone().try_into()?
        } else {
            home()?.join(".ssh/known_hosts")
        };

        let session = Session::new()?;
        let mut known_hosts = session.known_hosts()?;

        known_hosts.read_file(&host_file, KnownHostFileKind::OpenSSH)?;
        let all_hosts = known_hosts.hosts()?;
        let victims = all_hosts
            .iter()
            .filter(|host| cfg.host.test(host.name().unwrap_or("")))
            .filter(|host| cfg.key.test(host.key()))
            .collect::<Vec<_>>();
        let victim_count = victims.len();
        for v in victims {
            known_hosts.remove(v)?;
        }
        known_hosts.write_file(&host_file, KnownHostFileKind::OpenSSH)?;
        context.output.send(Value::Integer(victim_count as i128))
    }
}

pub fn declare(scope: &Scope) -> CrushResult<()> {
    scope.create_namespace(
        "remote",
        "Remote code execution",
        Box::new(move |remote| {
            Exec::declare(remote)?;
            Pexec::declare(remote)?;
            Identity::declare(remote)?;

            remote.create_namespace(
                "host",
                "Known remote hosts",
                Box::new(move |env| {
                    host::List::declare(env)?;
                    host::Remove::declare(env)?;
                    Ok(())
                }),
            )?;

            Ok(())
        }),
    )?;
    Ok(())
}
