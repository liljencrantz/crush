use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::completion::Completion;
use crate::lang::completion::parse::{LastArgument, PartialCommandResult};
use crate::lang::data::table::{ColumnType, Row};
use crate::lang::errors::{CrushError, CrushResult, CrushResultExtra, error, terminate};
use crate::lang::job_control::{ChannelBasedController, StreamControlMessage};
use crate::lang::state::global_state::GlobalState;
use crate::lang::serialization::{deserialize, serialize};
use crate::lang::signature::files::Files;
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::handles::CommandHandle;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::escape::{escape, escape_without_quotes};
use crate::util::file::home;
use crate::util::user_map::get_current_username;
use crossbeam::channel::{bounded, unbounded};
use signature::signature;
use ssh2::KnownHostFileKind;
use ssh2::{CheckResult, KnownHostKeyFormat, Session};
use std::cmp::min;
use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv6Addr, SocketAddr, TcpStream};
use std::path::PathBuf;

static IDENTITY_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("identity", ValueType::String),
    ColumnType::new("public_key", ValueType::Binary),
];

static HOST_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("host", ValueType::String),
    ColumnType::new("public_key", ValueType::String),
];

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
        username = default_username
            .clone()
            .unwrap_or(get_current_username()?.to_string());
    }

    // An IPv6 address is itself full of colons, so naively splitting `host` on ':' to
    // find a port breaks badly for it -- e.g. a bare "::1" would misparse as host=""
    // port="", and the conventional bracketed form "[::1]:2849" would misparse the
    // bracket itself as the host. Handle the IPv6 shapes explicitly, via std's own
    // address parsers rather than hand-rolled splitting, before falling through to the
    // plain split(':') logic that's already correct for a hostname/IPv4 address (which
    // never contain a literal ':').
    let port: u16;
    if let Ok(addr) = host.parse::<SocketAddr>() {
        // "[<ipv6>]:<port>" (also matches a plain IPv4 "a.b.c.d:port", already handled
        // correctly below, but this is simpler and gives the same result).
        port = addr.port();
        host = addr.ip().to_string();
    } else if let Ok(ip) = host.parse::<Ipv6Addr>() {
        // Bare "<ipv6>", no port, no brackets.
        port = 22;
        host = ip.to_string();
    } else if let Some(inner) = host
        .strip_prefix('[')
        .and_then(|s| s.strip_suffix(']'))
        .filter(|s| s.parse::<Ipv6Addr>().is_ok())
    {
        // Bracketed "[<ipv6>]", no port.
        port = 22;
        host = inner.to_string();
    } else if !host.contains(':') {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_user_at_host_defaults_to_port_22() {
        let (host, user, port) = parse("alice@example.com".to_string(), &None).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(user, "alice");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_parse_user_at_host_with_explicit_port() {
        let (host, user, port) = parse("alice@example.com:2222".to_string(), &None).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(user, "alice");
        assert_eq!(port, 2222);
    }

    #[test]
    fn test_parse_default_username_argument_used_when_host_has_no_at() {
        let (host, user, port) =
            parse("example.com".to_string(), &Some("bob".to_string())).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(user, "bob");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_parse_falls_back_to_current_username_with_no_at_and_no_default() {
        // No `@` in the host and no default_username given -- falls back to
        // get_current_username(), which should succeed on any machine this runs on.
        let (host, _user, port) = parse("example.com".to_string(), &None).unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_parse_invalid_port_errors() {
        assert!(parse("example.com:notaport".to_string(), &None).is_err());
    }

    #[test]
    fn test_parse_at_and_port_together() {
        let (host, user, port) =
            parse("root@10.0.0.1:2200".to_string(), &Some("ignored".to_string())).unwrap();
        assert_eq!(host, "10.0.0.1");
        assert_eq!(user, "root");
        assert_eq!(port, 2200);
    }

    #[test]
    fn test_parse_bare_ipv6_defaults_to_port_22() {
        let (host, _user, port) = parse("::1".to_string(), &None).unwrap();
        assert_eq!(host, "::1");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_parse_bracketed_ipv6_with_no_port_defaults_to_port_22() {
        let (host, _user, port) = parse("[::1]".to_string(), &None).unwrap();
        assert_eq!(host, "::1");
        assert_eq!(port, 22);
    }

    #[test]
    fn test_parse_bracketed_ipv6_with_explicit_port() {
        let (host, _user, port) = parse("[::1]:2849".to_string(), &None).unwrap();
        assert_eq!(host, "::1");
        assert_eq!(port, 2849);
    }

    #[test]
    fn test_parse_full_ipv6_with_at_and_bracketed_port() {
        let (host, user, port) = parse(
            "root@[2001:db8::1]:2222".to_string(),
            &Some("ignored".to_string()),
        )
        .unwrap();
        assert_eq!(host, "2001:db8::1");
        assert_eq!(user, "root");
        assert_eq!(port, 2222);
    }

    #[test]
    fn test_parse_ipv4_with_port_is_unaffected_by_the_ipv6_handling() {
        let (host, _user, port) = parse("10.0.0.1:22".to_string(), &None).unwrap();
        assert_eq!(host, "10.0.0.1");
        assert_eq!(port, 22);
    }
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
    command_handle: &CommandHandle,
) -> CrushResult<Value> {
    let (host, username, port) = parse(host, &default_username)?;

    // Not `format!("{}:{}", host, port)`: that reintroduces the exact bracket problem
    // `parse()` above just solved, since a bare IPv6 `host` (no brackets, as `parse()`
    // now always returns) glued directly to ":<port>" is ambiguous/unparseable as a
    // single string again. A (&str, u16) tuple's own ToSocketAddrs impl resolves the
    // host component directly -- hostname, IPv4, or bare IPv6 literal -- with no
    // string-level ambiguity at all.
    let tcp = TcpStream::connect((host.as_str(), port))?;
    let mut sess = Session::new()?;

    sess.set_tcp_stream(tcp);
    sess.handshake()?;

    if !ignore_host_file {
        let mut known_hosts = sess.known_hosts()?;
        known_hosts.read_file(host_file, KnownHostFileKind::OpenSSH)?;
        let (key, key_type) = sess
            .host_key()
            .ok_or(&format!("Could not fetch host key for {}", host))?;
        match known_hosts.check_port(&host, port, key) {
            CheckResult::Match => {}
            CheckResult::Mismatch => return error("Host mismatch"),
            CheckResult::NotFound => {
                if !allow_not_found {
                    return error(&format!("Host {} missing from known host file", host));
                } else {
                    let key_format: KnownHostKeyFormat = key_type.into();
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

    // A plain channel.read_to_end() here would be uninterruptible, exactly like a plain
    // std::thread::sleep() would be in `sleep` (see the comment there) -- so register a
    // controller and poll it the same way, reading in short-timeout chunks in between
    // instead of one single blocking read call. Session::set_timeout bounds libssh2's
    // own blocking wait for data (returning io::ErrorKind::TimedOut, not real failure,
    // once it elapses with nothing read) -- simpler than flipping the session
    // non-blocking and handling WouldBlock ourselves, since libssh2 still does the
    // actual waiting.
    const POLL_INTERVAL_MS: u32 = 50;
    let (control_sender, control_receiver) = bounded(1);
    command_handle.register(Box::from(ChannelBasedController::new(control_sender)));
    sess.set_timeout(POLL_INTERVAL_MS);
    let mut out_buf = Vec::new();
    let mut chunk = [0u8; 16384];
    loop {
        match channel.read(&mut chunk) {
            Ok(0) => {
                if channel.eof() {
                    break;
                }
            }
            Ok(n) => out_buf.extend_from_slice(&chunk[..n]),
            Err(e) if e.kind() == ErrorKind::TimedOut => {}
            Err(e) => return Err(e.into()),
        }
        match control_receiver.try_recv() {
            Ok(StreamControlMessage::Terminate) => return terminate(),
            Ok(StreamControlMessage::Pause) => loop {
                match control_receiver.recv() {
                    Ok(StreamControlMessage::Terminate) => return terminate(),
                    Ok(StreamControlMessage::Resume) => break,
                    Ok(StreamControlMessage::Pause) => {}
                    Err(_) => return terminate(),
                }
            },
            Ok(StreamControlMessage::Resume) | Err(_) => {}
        }
    }
    sess.set_timeout(0);

    let res = deserialize(&out_buf, env)?;
    channel.wait_close()?;
    Ok(res)
}

/// Report a single host's `run_remote` failure into the warning log rather than
/// aborting `pexec`. `err`'s own message usually says nothing about which host it came
/// from (e.g. a raw `TcpStream::connect` I/O error), so the host is folded into the
/// message text explicitly, the same way `fs:files`' own per-entry warnings do.
fn warn_connect_failure(global_state: &GlobalState, host: &str, err: CrushError) {
    global_state.warn(
        &error::<()>(format!(
            "Failed to run command on host {}. Reason: {}",
            host,
            err.message()
        ))
        .with_command("remote:pexec")
        .err()
        .unwrap(),
    );
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
                res.push(Completion::new(completion, host.name().unwrap_or(""), 0))
            }

            LastArgument::QuotedString(stripped_prefix) => {
                let completion = host.name().unwrap_or("");
                if completion.starts_with(stripped_prefix) && completion.len() > 0 {
                    res.push(Completion::new(
                        format!(
                            "{}\" ",
                            escape_without_quotes(&completion[stripped_prefix.len()..])
                        ),
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
    long = "Serializes `command` (a closure), sends it over SSH to `host`, runs it there in a",
    long = "fresh crush process, and returns its result. The remote host's key is checked",
    long = "against `host_file` unless `ignore_host_file` is set. Security note: setting",
    long = "`ignore_host_file` disables that check entirely (no protection against a",
    long = "different host answering at that address); `allow_not_found` instead trusts an",
    long = "unrecognized host's key on first use and saves it, rather than erroring -- both",
    long = "weaken protection against a machine-in-the-middle impersonating the remote host.",
    example = "remote:exec {host:name} \"my-server.example.com\" username=\"alice\"",
)]
struct Exec {
    #[description("the command to execute.")]
    command: Command,
    #[custom_completion(ssh_host_complete)]
    #[description("host to execute the command on.")]
    host: String,
    #[description("username on remote machines.")]
    username: Option<String>,
    #[description(
        "password on remote machines. If no password is provided, agent authentication will be used."
    )]
    password: Option<String>,
    #[description("(~/.ssh/known_hosts) known hosts file.")]
    host_file: Option<Files>,
    #[description("skip checking the know hosts file.")]
    #[default(false)]
    ignore_host_file: bool,
    #[description(
        "allow missing hosts in the known hosts file. Missing hosts will be automatically added to the file."
    )]
    #[default(false)]
    allow_not_found: bool,
}

fn exec(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Exec = Exec::parse(context.remove_arguments(), &context.global_state.printer())?;

    let host_file =
        crate::lang::signature::files::path(cfg.host_file, home()?.join(".ssh/known_hosts"))?;

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
        context.command_handle(),
    )?)
}

#[signature(
    remote.pexec,
    can_block = true,
    short = "Execute a command on a set of hosts",
    long = "Like `exec`, but runs `command` on every host listed in `host` (up to `parallel`",
    long = "of them at a time). pexec always attempts every host in the list, regardless of",
    long = "whether earlier hosts failed to connect or authenticate -- it never fails outright",
    long = "just because some, or even all, of the hosts couldn't be reached.",
    long = "A host that succeeds contributes one row (`host`/`result` columns) to the output;",
    long = "a host that fails contributes no row at all. Instead, the failure is logged as a",
    long = "warning (see `crush:warn:list`) naming the host and the underlying error. pexec's",
    long = "own exit status stays 0 either way, so the only way to tell whether every host",
    long = "succeeded is to compare the length of the output to the length of the `host` list",
    long = "you passed in -- fewer output rows than hosts means some connections failed.",
    long = "The same host-key verification applies independently to each host -- see `exec`",
    long = "for what `ignore_host_file`/`allow_not_found` mean for security.",
    example = "remote:pexec {host:name} \"web1.example.com\" \"web2.example.com\" username=\"alice\"",
    output = Known(ValueType::table_input_stream(&PEXEC_OUTPUT_TYPE)),
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
    #[description(
        "password on remote machines. If no password is provided, agent authentication will be used."
    )]
    password: Option<String>,
    #[description("(~/.ssh/known_hosts) known hosts file.")]
    host_file: Option<Files>,
    #[description("skip checking the know hosts file.")]
    #[default(false)]
    ignore_host_file: bool,
    #[description(
        "allow missing hosts in the known hosts file. Missing hosts will be automatically added to the file."
    )]
    #[default(false)]
    allow_not_found: bool,
}

static PEXEC_OUTPUT_TYPE: [ColumnType; 2] = [
    ColumnType::new("host", ValueType::String),
    ColumnType::new("result", ValueType::Any),
];

fn pexec(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Pexec = Pexec::parse(context.remove_arguments(), &context.global_state.printer())?;
    let host_file =
        crate::lang::signature::files::path(cfg.host_file, home()?.join(".ssh/known_hosts"))?;

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
        let my_global_state = context.global_state.clone();
        let my_command_handle = context.next_command_handle();
        let thread_command_handle = my_command_handle.clone();

        context.global_state.threads().spawn(
            "remote:pexec",
            &my_command_handle,
            move || {
                let my_command_handle = thread_command_handle;
                while let Ok(host) = my_recv.recv() {
                    // A failure here must not propagate via `?`: that would unwind this
                    // whole worker thread out of its loop, permanently pulling it out of
                    // the shared pool -- any host still queued that no other worker
                    // happens to claim first would then never be attempted at all, not
                    // just never reported. Report it as a warning and keep going instead.
                    match run_remote(
                        &my_buf,
                        &my_env,
                        host.clone(),
                        &my_username,
                        &my_password,
                        &my_host_file,
                        my_ignore_host_file,
                        my_allow_not_found,
                        &my_command_handle,
                    ) {
                        Ok(res) => my_send.send((host, res))?,
                        Err(err) => warn_connect_failure(&my_global_state, &host, err),
                    }
                }
                Ok(())
            },
        )?;
    }

    drop(result_send);
    let output = context.initialize_output(&PEXEC_OUTPUT_TYPE)?;

    while let Ok((host, val)) = result_recv.recv() {
        output.send(Row::new(vec![Value::from(host), val]))?;
    }

    Ok(())
}

#[signature(
    remote.identity,
    can_block = true,
    output = Known(ValueType::table_input_stream(&IDENTITY_OUTPUT_TYPE)),
    short = "List all known ssh-agent identities"
)]
struct Identity {}

fn identity(context: CommandContext) -> CrushResult<()> {
    let output = context.initialize_output(&IDENTITY_OUTPUT_TYPE)?;
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

    #[signature(
        remote.host.list,
        can_block = true,
        output = Known(ValueType::table_input_stream(&HOST_OUTPUT_TYPE)),
        short = "List all known hosts",
        long = "If a given host key has no hostname, the hostname will be the empty string"
    )]
    pub struct List {
        #[description("(~/.ssh/known_hosts) known hosts file.")]
        host_file: Option<Files>,
    }

    fn list(mut context: CommandContext) -> CrushResult<()> {
        let cfg: List = List::parse(context.remove_arguments(), &context.global_state.printer())?;
        let output = context.initialize_output(&HOST_OUTPUT_TYPE)?;
        let session = Session::new()?;

        let mut known_hosts = session.known_hosts()?;

        // Initialize the known hosts with a global known hosts file
        let host_file =
            crate::lang::signature::files::path(cfg.host_file, home()?.join(".ssh/known_hosts"))?;
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
        host_file: Option<Files>,
        #[description("host filter.")]
        host: Patterns,
        #[description("key filter.")]
        key: Patterns,
    }

    fn remove(mut context: CommandContext) -> CrushResult<()> {
        let cfg: Remove =
            Remove::parse(context.remove_arguments(), &context.global_state.printer())?;
        let host_file =
            crate::lang::signature::files::path(cfg.host_file, home()?.join(".ssh/known_hosts"))?;

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
        Some(
            "Commands for running code on other machines over SSH: `remote:exec` runs a \
             closure on one host, `remote:pexec` runs it across several in parallel, and \
             `remote:identity` lists the identities your ssh-agent has loaded. `remote:host` \
             tracks known-hosts entries, the same trust store `ssh` itself uses."
                .to_string(),
        ),
        Box::new(move |remote| {
            Exec::declare(remote)?;
            Pexec::declare(remote)?;
            Identity::declare(remote)?;

            remote.create_namespace(
                "host",
                "Known remote hosts",
                None,
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
