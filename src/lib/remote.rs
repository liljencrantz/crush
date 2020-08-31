use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{error, mandate, to_crush_error, CrushError, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::files::Files;
use crate::lang::patterns::Patterns;
use crate::lang::data::scope::Scope;
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::file::home;
use crossbeam::unbounded;
use lazy_static::lazy_static;
use signature::signature;
use ssh2::KnownHostFileKind;
use ssh2::{CheckResult, HostKeyType, KnownHostKeyFormat, Session};
use std::cmp::min;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::thread::JoinHandle;
use users::get_current_username;

lazy_static! {
    static ref IDENTITY_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("identity", ValueType::String),
        ColumnType::new("key", ValueType::Binary),
    ];
    static ref HOST_LIST_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("host", ValueType::String),
        ColumnType::new("key", ValueType::String),
    ];
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
        username = default_username.clone().unwrap_or(
            mandate(
                mandate(
                    get_current_username(),
                    "Could not determine current username",
                )?
                .to_str(),
                "Invalid username",
            )?
            .to_string(),
        );
    }

    let port: u16;
    if !host.contains(':') {
        port = 22;
    } else {
        let mut parts = host.split(':');
        let tmp = parts.next().unwrap().to_string();
        port = to_crush_error(parts.next().unwrap().parse::<u16>())?;
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

    let tcp = to_crush_error(TcpStream::connect(&format!("{}:{}", host, port)))?;
    let mut sess = to_crush_error(Session::new())?;

    sess.set_tcp_stream(tcp);
    to_crush_error(sess.handshake())?;

    if !ignore_host_file {
        let mut known_hosts = to_crush_error(sess.known_hosts())?;
        to_crush_error(known_hosts.read_file(host_file, KnownHostFileKind::OpenSSH))?;
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
                    to_crush_error(known_hosts.add(&host, key, "Added by Crush", key_format))?;
                    to_crush_error(known_hosts.write_file(host_file, KnownHostFileKind::OpenSSH))?;
                }
            }
            CheckResult::Failure => return error("Host validation check failure"),
        }
    }

    if let Some(pass) = password {
        to_crush_error(sess.userauth_password(&username, pass))?
    } else {
        to_crush_error(sess.userauth_agent(&username))?;
    }

    let mut channel = to_crush_error(sess.channel_session())?;
    to_crush_error(channel.exec("crush --pup"))?;
    to_crush_error(channel.write(cmd))?;
    to_crush_error(channel.send_eof())?;
    let mut out_buf = Vec::new();
    to_crush_error(channel.read_to_end(&mut out_buf))?;
    let res = deserialize(&out_buf, env)?;
    to_crush_error(channel.wait_close())?;
    Ok(res)
}

#[signature(
    exec,
    can_block = true,
    short = "Execute a command on a host",
    long = "    Execute the specified command on the soecified host"
)]
struct Exec {
    #[description("the command to execute.")]
    command: Command,
    #[description("host to execute the command on.")]
    host: String,
    #[description("username on remote machines.")]
    username: Option<String>,
    #[description("password on remote machines. If no password is provided, agent authentication will be used.")]
    password: Option<String>,
    #[description("(~/.ssh/known_hosts) known hosts file.")]
    host_file: Files,
    #[description("skip checking the know hosts file.")]
    #[default(false)]
    ignore_host_file: bool,
    #[description("allow missing hosts in the known hosts file. Missing hosts will be automatically added to the file.")]
    #[default(false)]
    allow_not_found: bool,
}

fn exec(context: CommandContext) -> CrushResult<()> {
    let cfg: Exec = Exec::parse(context.arguments, &context.printer)?;
    let host_file = if cfg.host_file.had_entries() {
        cfg.host_file.into_file()?
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
    pexec,
    can_block = true,
    short = "Execute a command on a set of hosts",
    long = "    Execute the specified command all specified hosts"
)]
struct Pexec {
    #[description("the command to execute.")]
    #[description("the command to execute.")]
    command: Command,
    #[unnamed()]
    #[description("hosts to execute the command on.")]
    host: Vec<String>,
    #[description("maximum number of hosts to run on in parallel.")]
    #[default(32)]
    parallel: i128,
    #[description("username on remote machines.")]
    username: Option<String>,
    #[description("password on remote machines. If no password is provided, agent authentication will be used.")]
    password: Option<String>,
    #[description("(~/.ssh/known_hosts) known hosts file.")]
    host_file: Files,
    #[description("skip checking the know hosts file.")]
    #[default(false)]
    ignore_host_file: bool,
    #[description("allow missing hosts in the known hosts file. Missing hosts will be automatically added to the file.")]
    #[default(false)]
    allow_not_found: bool,
}

fn pexec(context: CommandContext) -> CrushResult<()> {
    let cfg: Pexec = Pexec::parse(context.arguments, &context.printer)?;
    let host_file = if cfg.host_file.had_entries() {
        cfg.host_file.into_file()?
    } else {
        home()?.join(".ssh/known_hosts")
    };

    let (host_send, host_recv) = unbounded::<String>();
    let (result_send, result_recv) = unbounded::<(String, Value)>();

    let mut in_buf = Vec::new();

    serialize(&Value::Command(cfg.command), &mut in_buf)?;

    for host in &cfg.host {
        to_crush_error(host_send.send(host.clone()))?;
    }

    drop(host_send);

    let thread_count = min(cfg.parallel as usize, cfg.host.len());
    let mut threads: Vec<JoinHandle<std::result::Result<(), CrushError>>> =
        Vec::with_capacity(thread_count);
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

        let t: JoinHandle<std::result::Result<(), CrushError>> = to_crush_error(
            thread::Builder::new()
                .name("remote:pexec".to_string())
                .spawn(move || {
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
                        to_crush_error(my_send.send((host, res)))?;
                    }
                    Ok(())
                }),
        )?;
        threads.push(t);
    }

    drop(result_send);
    let output = context.output.initialize(vec![
        ColumnType::new("host", ValueType::String),
        ColumnType::new("result", ValueType::Any),
    ])?;

    while let Ok((host, val)) = result_recv.recv() {
        output.send(Row::new(vec![Value::String(host), val]))?;
    }

    for t in threads {
        match t.join() {
            Ok(res) => {
                res?;
            }
            Err(_) => {
                return error("Unknown error while waiting for thread in remote:exec");
            }
        }
    }

    Ok(())
}

#[signature(
identity,
can_block = true,
output = Known(ValueType::TableStream(IDENTITY_OUTPUT_TYPE.clone())),
short = "List all known ssh-agent identities"
)]
struct Identity {}

fn identity(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(IDENTITY_OUTPUT_TYPE.clone())?;
    let sess = to_crush_error(Session::new())?;
    let mut agent = to_crush_error(sess.agent())?;

    to_crush_error(agent.connect())?;
    to_crush_error(agent.list_identities())?;

    for identity in to_crush_error(agent.identities())? {
        output.send(Row::new(vec![
            Value::String(identity.comment().to_string()),
            Value::Binary(identity.blob().to_vec()),
        ]))?;
    }
    Ok(())
}

mod host {
    use super::*;

    #[signature(
    list,
    can_block = true,
    output = super::Known(ValueType::TableStream(super::HOST_LIST_OUTPUT_TYPE.clone())),
    short = "List all known hosts",
    long = "If a given host key has no hostname, the hostname will be the empty string"
    )]
    pub struct List {
        #[description("(~/.ssh/known_hosts) known hosts file.")]
        host_file: Files,
    }

    fn list(context: CommandContext) -> CrushResult<()> {
        let cfg: List = List::parse(context.arguments, &context.printer)?;
        let output = context
            .output
            .initialize(super::HOST_LIST_OUTPUT_TYPE.clone())?;
        let session = to_crush_error(Session::new())?;

        let mut known_hosts = to_crush_error(session.known_hosts())?;

        // Initialize the known hosts with a global known hosts file
        let host_file = if cfg.host_file.had_entries() {
            cfg.host_file.into_file()?
        } else {
            home()?.join(".ssh/known_hosts")
        };
        to_crush_error(known_hosts.read_file(&host_file, KnownHostFileKind::OpenSSH))?;
        for host in to_crush_error(known_hosts.iter())? {
            output.send(Row::new(vec![
                Value::String(host.name().unwrap_or("").to_string()),
                Value::String(host.key().to_string()),
            ]))?;
        }
        Ok(())
    }

    #[signature(
    remove,
    can_block = true,
    short = "Remove hosts from known_hosts file",
    output = Known(ValueType::Integer),
    long = "Remove all hosts that match both the host and the key filters."
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
        let cfg: Remove = Remove::parse(context.arguments, &context.printer)?;
        let host_file = if cfg.host_file.had_entries() {
            cfg.host_file.into_file()?
        } else {
            home()?.join(".ssh/known_hosts")
        };

        let session = to_crush_error(Session::new())?;
        let mut known_hosts = to_crush_error(session.known_hosts())?;

        to_crush_error(known_hosts.read_file(&host_file, KnownHostFileKind::OpenSSH))?;
        let all_hosts = to_crush_error(known_hosts.hosts())?;
        let victims = all_hosts
            .iter()
            .filter(|host| cfg.host.test(host.name().unwrap_or("")))
            .filter(|host| cfg.key.test(host.key()))
            .collect::<Vec<_>>();
        let victim_count = victims.len();
        for v in victims {
            to_crush_error(known_hosts.remove(v))?;
        }
        to_crush_error(known_hosts.write_file(&host_file, KnownHostFileKind::OpenSSH))?;
        context.output.send(Value::Integer(victim_count as i128))
    }
}

pub fn declare(scope: &Scope) -> CrushResult<()> {
    let e = scope.create_namespace(
        "remote",
        Box::new(move |remote| {
            Exec::declare(remote)?;
            Pexec::declare(remote)?;
            Identity::declare(remote)?;

            remote.create_namespace(
                "host",
                Box::new(move |env| {
                    host::List::declare(env)?;
                    host::Remove::declare(env)?;
                    Ok(())
                }),
            )?;

            Ok(())
        }),
    )?;
    scope.r#use(&e);
    Ok(())
}
