//! `VtSession` -- a real pty-backed child process plus everything needed to drive and
//! inspect it from a script. Lives in a private, global `id -> Option<VtSession>`
//! registry (`SESSIONS` below), exactly like `GrpcClient` in
//! `src/builtins/grpc/client.rs`: scripts never hold a Rust handle to this directly, only
//! the plain integer id embedded in the `Struct` `vt:create` returns (see `create.rs`).

use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::patterns::Patterns;
use chrono::Duration;
use nix::libc;
use nix::pty::{OpenptyResult, Winsize, openpty};
use nix::sys::signal::{Signal, kill};
use nix::unistd::{Pid as NixPid, setsid};
use parking_lot::{MappedRwLockReadGuard, Mutex, RwLock, RwLockReadGuard};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::LazyLock;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration as StdDuration, Instant};

static SESSIONS: LazyLock<RwLock<HashMap<i32, Option<VtSession>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static NEXT_ID: AtomicI32 = AtomicI32::new(1);

/// Re-check interval while `wait_for` polls -- matches the interval the now-removed
/// tests/interactive_harness.rs prototype already validated as responsive without
/// busy-spinning.
const POLL_INTERVAL: StdDuration = StdDuration::from_millis(20);

pub struct VtSession {
    pid: i32,
    master: Mutex<std::fs::File>,
    child: Mutex<Child>,
    rx: Mutex<Receiver<Vec<u8>>>,
    parser: Mutex<vt100::Parser>,
}

impl VtSession {
    fn sessions() -> &'static RwLock<HashMap<i32, Option<VtSession>>> {
        &SESSIONS
    }

    /// Spawns `program` (with `args`) attached to a fresh pty of `rows`x`cols`, as the
    /// pty's session leader and controlling-terminal owner -- without that, job-control
    /// signals (Ctrl-Z, etc.) sent through this terminal wouldn't behave like a real one
    /// at all. Registers the resulting session and returns its id.
    pub fn create(program: &Path, args: &[String], rows: u16, cols: u16) -> CrushResult<i32> {
        let winsize = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let OpenptyResult { master, slave } = openpty(&winsize, None)?;

        let ioctl_fd = slave.as_raw_fd();
        let slave_stdin = slave.try_clone()?;
        let slave_stdout = slave.try_clone()?;
        let slave_stderr = slave;

        let mut cmd = Command::new(program);
        cmd.args(args)
            .env("TERM", "xterm")
            .stdin(Stdio::from(slave_stdin))
            .stdout(Stdio::from(slave_stdout))
            .stderr(Stdio::from(slave_stderr));
        unsafe {
            cmd.pre_exec(move || {
                setsid().map_err(std::io::Error::from)?;
                if libc::ioctl(ioctl_fd, libc::TIOCSCTTY as _, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                libc::close(ioctl_fd);
                Ok(())
            });
        }
        let child = cmd.spawn()?;
        let pid = child.id() as i32;

        let master_reader = master.try_clone()?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let mut f = std::fs::File::from(master_reader);
            let mut buf = [0u8; 4096];
            loop {
                match f.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    // EIO once every slave-side descriptor is closed -- the normal way
                    // this loop ends, same as a plain pipe's Ok(0) EOF.
                    Err(_) => break,
                }
            }
        });

        let session = VtSession {
            pid,
            master: Mutex::new(std::fs::File::from(master)),
            child: Mutex::new(child),
            rx: Mutex::new(rx),
            parser: Mutex::new(vt100::Parser::new(rows, cols, 0)),
        };

        let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
        let mut map = Self::sessions().write();
        map.insert(id, Some(session));
        Ok(id)
    }

    pub fn close(id: i32) {
        let mut map = Self::sessions().write();
        if let Some(slot) = map.get_mut(&id) {
            *slot = None;
        }
    }

    /// Tears down every still-open session -- called exactly once, unconditionally, near
    /// the end of `main.rs`'s `run()`, so a script that forgot `:close` (or errored out
    /// before reaching it) never leaves a real child process running past the end of the
    /// crush process that created it.
    pub fn close_all() {
        let mut map = Self::sessions().write();
        for slot in map.values_mut() {
            *slot = None;
        }
    }

    pub fn get(s: &Struct) -> CrushResult<MappedRwLockReadGuard<'static, VtSession>> {
        match s.get("id") {
            Some(crate::lang::value::Value::Integer(id)) => Self::get_from_id(id as i32),
            _ => command_error("Missing or invalid id field in vt method object"),
        }
    }

    pub fn get_from_id(id: i32) -> CrushResult<MappedRwLockReadGuard<'static, VtSession>> {
        let res = RwLockReadGuard::try_map(Self::sessions().read(), |map| match map.get(&id) {
            Some(Some(session)) => Some(session),
            _ => None,
        });
        match res {
            Ok(session) => Ok(session),
            _ => command_error(format!(
                "Unknown vt session id {}. Did you already close it?",
                id
            )),
        }
    }

    pub fn write_raw(&self, text: &str) -> CrushResult<()> {
        let mut master = self.master.lock();
        master.write_all(text.as_bytes())?;
        master.flush()?;
        Ok(())
    }

    fn drain_available(&self) {
        let mut parser = self.parser.lock();
        let rx = self.rx.lock();
        while let Ok(chunk) = rx.try_recv() {
            parser.process(&chunk);
        }
    }

    /// The current rendered screen, trailing blank lines trimmed -- `vt100` always
    /// renders its full, fixed-size row/column grid, padding every unused row with blank
    /// lines, so trimming keeps a `wait_for` timeout's attached screen dump (and any
    /// script that prints `:contents` directly) readable instead of mostly blank lines.
    pub fn contents(&self) -> String {
        self.drain_available();
        let parser = self.parser.lock();
        let contents = parser.screen().contents();
        let trimmed_len = contents
            .lines()
            .rev()
            .skip_while(|l| l.trim().is_empty())
            .count();
        contents
            .lines()
            .take(trimmed_len)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Blocks (processing new output as it arrives) until the rendered screen matches
    /// `pattern` (substring for a plain string, see `Patterns::test_contains`; glob/regex
    /// matched against the whole screen as usual), or returns an `Err` with the current
    /// screen attached once `timeout` elapses.
    pub fn wait_for(&self, pattern: &Patterns, timeout: Duration) -> CrushResult<()> {
        let deadline =
            Instant::now() + timeout.to_std().unwrap_or(StdDuration::from_secs(5));
        loop {
            let contents = self.contents();
            if pattern.test_contains(&contents) {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return command_error(format!(
                    "Timed out waiting for {} to appear.\n--- rendered screen ---\n{}\n--- end rendered screen ---",
                    pattern, contents,
                ));
            }
            // Lock ordering must match `drain_available` (`parser` before `rx`, never the
            // reverse) to avoid an AB-BA deadlock between this poll loop and a concurrent
            // `:contents()`/`drain_available` call on the same session -- so the `rx`
            // guard is dropped (via this `let` binding finishing before the `if let`
            // starts, rather than being kept alive for the whole `if let` body the way a
            // bare `if let Ok(chunk) = self.rx.lock().recv_timeout(..)` would) strictly
            // before `parser` is locked below.
            let received = self.rx.lock().recv_timeout(POLL_INTERVAL);
            if let Ok(chunk) = received {
                self.parser.lock().process(&chunk);
            }
        }
    }

    pub fn pid(&self) -> i32 {
        self.pid
    }

    pub fn alive(&self) -> CrushResult<bool> {
        let mut child = self.child.lock();
        Ok(child.try_wait()?.is_none())
    }
}

impl Drop for VtSession {
    fn drop(&mut self) {
        kill_process_tree(self.pid as u32);
        let _ = self.child.lock().wait();
    }
}

/// Every `cmd`-spawned external command gets its own process group, so a backgrounded
/// one touching the terminal doesn't take the session's own shell down with it -- which
/// also means a plain "kill this one pid" would miss any such child entirely. Walking the
/// real parent/child process tree is what actually reaches all of them.
fn kill_process_tree(root_pid: u32) {
    let mut sys = sysinfo::System::new_all();
    sys.refresh_all();

    let mut targets = HashSet::new();
    let mut frontier = vec![root_pid];
    while let Some(pid) = frontier.pop() {
        if !targets.insert(pid) {
            continue;
        }
        for (candidate_pid, proc) in sys.processes() {
            if proc.parent().map(|p| p.as_u32()) == Some(pid) {
                frontier.push(candidate_pid.as_u32());
            }
        }
    }
    for pid in targets {
        let _ = kill(NixPid::from_raw(pid as i32), Signal::SIGKILL);
    }
}
