# `vt` Virtual Terminal Builtin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `vt` builtin namespace that spawns a PTY-backed child process and exposes it to crush scripts (`vt:create`, `$t:write`, `$t:wait_for`, `$t:contents`, `$t:pid`, `$t:alive`, `$t:close`, `$t:control:*`), then use it to write a `tests/vt_*.crush` golden-test battery covering Ctrl-Z, `fg`, `bg`, `terminate`, and `host:signal` job-control behavior, fixing any real crush bugs the tests surface.

**Architecture:** A private `VtSession` resource (pty master, child process, reader thread, `vt100::Parser`) lives in a global `id -> Option<VtSession>` registry, exactly mirroring `src/builtins/grpc/client.rs`'s `GrpcClient`. `vt:create` returns a plain `Struct` with one `{id, __call__}` sub-struct per method (mirroring `grpc:connect`'s per-RPC-method sub-structs) plus a `control` sub-struct of named control-character constants. No new `ValueType` is introduced.

**Tech Stack:** Rust, `nix` (pty, `term` feature — already a dependency), `vt100` (new dependency, real VT100/ANSI emulation), `sysinfo` (already a dependency, process-tree cleanup), `parking_lot` (already a dependency, locking), the project's own `signature` macro and `ordered_map` crate.

**Spec:** `docs/superpowers/specs/2026-10-10-vt-virtual-terminal-design.md`

## Global Constraints

- No new `ValueType`. Methods are plain `Struct` fields of `{id, __call__}`, resolved through a private `static SESSIONS: LazyLock<RwLock<HashMap<i32, Option<VtSession>>>>` registry, exactly like `GrpcClient` (`src/builtins/grpc/client.rs`).
- `wait_for`'s pattern argument accepts a plain string, a glob, or a regex (the `Patterns` signature type, as used by `grpc:connect`'s `service` field) — not a bespoke string-only API.
- There is no `:key` method. Control characters are sent as their literal byte through `write`; readability is provided by a `control` struct of named one-character constants (`control:c`, `control:d`, `control:z`), reachable both at `vt:control:*` and `$t:control:*`.
- A `VtSession` must never outlive the crush process that created it: `VtSession::close_all()` is called unconditionally once near the end of `src/main.rs`'s `run()`, regardless of which `Mode` ran or whether the script called `exit` — not only from an explicit `close`/`exit --force` path.
- New tests are plain `tests/vt_*.crush` + `tests/vt_*.crush.output` golden-file pairs, auto-discovered by the *existing*, unmodified `test_finder!()` call in `tests/system.rs` — no new test format.
- The in-progress, uncommitted Rust PTY harness this supersedes (`tests/interactive_harness.rs`, `tests/interactive.rs`, `tests/interactive/*.itest`, the `test_finder` `(dir, ext, runner)` generalization, the `vt100` dev-dependency, the `[[test]] name = "interactive"` Cargo.toml entry) is removed as part of this plan, not left alongside the new code.

## Review Focus

- A `.crush` test (or any script) that errors out mid-run after `vt:create` but before `:close` must not leave the spawned child process running once the crush process that created it exits — this is the entire reason the exit-time `close_all()` hook exists; a test exercising "script dies mid-way, no explicit `:close`" is as important as the happy path.
- Calling any method on a `vt` session struct *after* `:close` has already been called on it (or on an id that never existed) must return a normal `CrushResult::Err` ("Unknown vt session id ... Did you already close it?"), not panic — a script holding onto a stale sub-struct is an easy mistake to make.
- `wait_for`'s plain-string pattern must mean *substring containment* ("did this text ever appear anywhere on the growing screen"), matching the entire point of the feature — **not** the whole-string-equality semantics `Value::String::matches`/`:like` use elsewhere in the language (see Task 2, Step 2 below: `Patterns::test()` is exact-match by design and must not be reused as-is here).
- `vt:create` given a program that doesn't exist, or an empty command, must return a clean error, not panic — a test author will absolutely typo a binary name while writing these.
- Writing a control character (e.g. `$t:control:z`) must actually be observable as the *named* job-control effect (job becomes "Paused" in the nested shell's own `jobs` listing), not merely "the byte was written without erroring" — the battery tests (Tasks 5-10) assert on the downstream job-control effect, not just that `write` succeeded.

---

## File Structure

```
src/builtins/vt/
  mod.rs       — declare(root): wires `vt:create` and the `vt:control` constant into the namespace;
                 exposes pub fn close_all_sessions() for main.rs to call on exit.
  session.rs   — VtSession (the pty/child/parser resource) + the private id registry.
  methods.rs   — the per-session method structs/handlers: write, wait_for, contents, pid, alive, close.
  create.rs    — the Create (`vt:create`) struct/handler that builds the returned Struct.
  control.rs   — the shared `control` constants Struct (c/d/z).

src/lang/signature/patterns.rs   — gains Patterns::test_contains (substring/glob/regex "appears
                                    anywhere", distinct from the existing exact-match test()).
src/builtins/mod.rs              — mod vt; becomes pub mod vt; wt::declare(root)? added.
src/main.rs                      — close_all_sessions() called once, unconditionally, near the end
                                    of run().
tests/system.rs                  — one new std::env::set_var("CRUSH_BIN", ...) in the existing
                                    start_test_servers() ctor, so .crush tests can spawn the real
                                    binary under test via crush:env["CRUSH_BIN"].
tests/vt_*.crush / .crush.output — the new golden tests.
```

---

### Task 1: Remove the superseded Rust PTY-harness prototype

**Files:**
- Revert: `Cargo.toml`, `test_finder/src/lib.rs` (both to their committed `HEAD` content)
- Delete: `tests/interactive.rs`, `tests/interactive_harness.rs`, `tests/interactive/` (and everything under it)

**Interfaces:** None — this task only removes code, producing the clean baseline every later task builds on.

- [ ] **Step 1: Confirm the diffs are exactly the prototype's own additions, then revert**

```bash
git diff HEAD -- Cargo.toml test_finder/src/lib.rs
```

Expected: the `Cargo.toml` diff is only the `vt100 = "0.16"` dev-dependency line and the
`[[test]] name = "interactive"` block; the `test_finder/src/lib.rs` diff is only the
`(dir, ext, runner)` generalization. (Both files are otherwise unmodified since `HEAD` —
this has already been verified while writing this plan.) Then:

```bash
git checkout HEAD -- Cargo.toml test_finder/src/lib.rs
rm -f tests/interactive.rs tests/interactive_harness.rs
rm -rf tests/interactive
git status --porcelain
```

Expected final `git status --porcelain` output: no trace of `Cargo.toml`, `test_finder/`,
or `tests/interactive*` (they were untracked, so deleting them leaves nothing behind;
the other two are back to their committed content, i.e. show as unmodified).

- [ ] **Step 2: Confirm the baseline is clean**

```bash
cargo build && cargo test
```

Expected: builds and passes exactly as it did before any of this session's `vt`-related
work started — this is the baseline every later task's own test run is compared against.

- [ ] **Step 3: Nothing to commit**

There is nothing to commit here: Step 1 restored `Cargo.toml` and `test_finder/src/lib.rs`
to their already-committed `HEAD` content (`git diff HEAD` on both is now empty), and the
deleted files were never tracked in the first place. Confirm with `git status --porcelain`
(already done in Step 1) and move straight to Task 2.

---

### Task 2: `VtSession` resource + registry, and the `write`/`wait_for`/`contents`/`close` vertical slice

**Files:**
- Create: `src/builtins/vt/mod.rs`
- Create: `src/builtins/vt/session.rs`
- Create: `src/builtins/vt/methods.rs`
- Create: `src/builtins/vt/create.rs`
- Modify: `src/lang/signature/patterns.rs` (add `test_contains`)
- Modify: `src/builtins/mod.rs:26-30,52-56` (register the new module)
- Modify: `Cargo.toml` (move `vt100` into `[dependencies]`)
- Test: `tests/vt_smoke.crush` + `tests/vt_smoke.crush.output`

**Interfaces:**
- Consumes: `nix::pty::{openpty, Winsize, OpenptyResult}`, `nix::unistd::setsid`, `nix::libc`,
  `sysinfo::System`, `nix::sys::signal::{kill, Signal}`, `vt100::Parser`,
  `crate::lang::signature::patterns::Patterns`, `crate::lang::state::this::This`,
  `crate::lang::data::r#struct::Struct`, `crate::lang::command::{Command, CrushCommand, OutputType::Known}`,
  `ordered_map::OrderedMap`.
- Produces (used by Task 3 onward):
  `VtSession::create(program: &std::path::Path, args: &[String], rows: u16, cols: u16) -> CrushResult<i32>`,
  `VtSession::close(id: i32)`, `VtSession::close_all()`,
  `VtSession::get(s: &Struct) -> CrushResult<MappedRwLockReadGuard<'static, VtSession>>`,
  `VtSession::get_from_id(id: i32) -> CrushResult<MappedRwLockReadGuard<'static, VtSession>>`,
  `VtSession::write_raw(&self, text: &str) -> CrushResult<()>`,
  `VtSession::contents(&self) -> String`,
  `VtSession::wait_for(&self, pattern: &Patterns, timeout: chrono::Duration) -> CrushResult<()>`,
  `VtSession::pid(&self) -> i32`, `VtSession::alive(&self) -> CrushResult<bool>`,
  `methods::methods() -> &'static OrderedMap<String, Command>` (keys: `write`, `wait_for`, `contents`, `close`
  in this task; `pid`, `alive` added in Task 3),
  `crate::builtins::vt::close_all_sessions()`.

- [ ] **Step 1: Write the test first**

```
# tests/vt_smoke.crush
# Smoke test for the vt builtin itself: spawn a trivial external command, drive it, and
# check its output through a real pty -- `cat` simply echoes back whatever it reads, which
# a pty's own line discipline (in its default cooked mode) already echoes too, so "hello"
# is guaranteed to show up on screen one way or another once cat has had a chance to run.
$t := (vt:create cat)
$t:write "hello\n"
$t:wait_for "hello"
$t:close
echo "ok"
```

```
# tests/vt_smoke.crush.output
ok
```

- [ ] **Step 2: Run it to see it fail (there is no `vt` namespace yet)**

```bash
cargo build && cargo test --test system test_vt_smoke
```

Expected: FAIL — `cargo build` itself fails once `vt100` is referenced from Step 3 but not
yet a real dependency, or (before touching Cargo.toml) the test fails with a `vt` namespace
not found/unknown command error. Either failure is fine; the point is confirming there's
nothing already providing this.

- [ ] **Step 3: Move `vt100` into real dependencies**

In `Cargo.toml`, delete the `vt100 = "0.16"` line from `[dev-dependencies]` and add it to
the main `[dependencies]` block instead (anywhere in that list; alphabetically it reads
naturally right after `unicode-width`):

```toml
[dependencies]
...
unicode-width = "0.2.2"
vt100 = "0.16"
os_pipe = "1.2.3"
...
```

(`nix`'s existing `features = [..., "term"]` already covers `nix::pty`; `sysinfo` and
`parking_lot` are already plain `[dependencies]` entries — no other `Cargo.toml` change is
needed.)

- [ ] **Step 4: Add `Patterns::test_contains`**

`Patterns::test()` (used by `grpc:connect`'s `service: Patterns` field) does *exact*
whole-string matching for a plain string pattern — correct for "does this service name
equal X", wrong for "did this text ever appear anywhere on the rendered screen". Add a
second method rather than changing `test()`'s existing behavior:

In `src/lang/signature/patterns.rs`, add:

```rust
impl Patterns {
    // ... existing new/expand_string/expand_glob/expand_regex/test/is_empty stay as-is ...

    /// Like `test()`, but a plain string pattern means "appears somewhere in `haystack`"
    /// rather than "equals `haystack` exactly" -- the semantics `vt:wait_for` needs (did
    /// this text ever get printed on a screen that also has lots of *other* text on it),
    /// as opposed to `test()`'s exact-match semantics (e.g. "is this one service name").
    /// A glob or regex pattern already supports "appears somewhere" on its own (an
    /// unanchored regex, or a glob the caller wrote with explicit `*...*` wildcards), so
    /// only the plain-string case needs different handling here.
    pub fn test_contains(&self, haystack: &str) -> bool {
        for v in &self.patterns {
            let matched = match v {
                Value::String(s) => haystack.contains(&s.to_string()),
                Value::Glob(g) => g.matches(haystack),
                Value::Regex(_, re) => re.is_match(haystack),
                _ => false,
            };
            if matched {
                return true;
            }
        }
        false
    }
}
```

- [ ] **Step 5: Write `src/builtins/vt/session.rs`**

```rust
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
    rx: Receiver<Vec<u8>>,
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
            rx,
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
        while let Ok(chunk) = self.rx.try_recv() {
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
            if let Ok(chunk) = self.rx.recv_timeout(POLL_INTERVAL) {
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
```

- [ ] **Step 6: Write `src/builtins/vt/methods.rs`**

```rust
use crate::builtins::vt::session::VtSession;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::signature::patterns::Patterns;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use chrono::Duration;
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Write::declare_method(&mut res);
        WaitFor::declare_method(&mut res);
        Contents::declare_method(&mut res);
        Close::declare_method(&mut res);
        res
    })
}

#[signature(
    vt.session.write,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Write literal text to this virtual terminal, as if typed at the keyboard.",
    long = "No newline or key translation is added -- include `\\n` explicitly to submit a",
    long = "command line, and use `$t:control:<name>` (see `vt:control`) for control",
    long = "characters such as Ctrl-Z.",
    example = "$t:write \"echo hello\\n\"",
    example = "$t:write $t:control:z",
)]
struct Write {
    #[description("the literal text to send.")]
    text: String,
}

fn write(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Write = Write::parse(context.remove_arguments(), &context.global_state.printer())?;
    let session = VtSession::get(&context.this.r#struct()?)?;
    session.write_raw(&cfg.text)?;
    context.output.send(Value::Empty)
}

#[signature(
    vt.session.wait_for,
    can_block = true,
    output = Known(ValueType::Empty),
    short = "Block until the virtual terminal's rendered screen contains a pattern, or error out after a timeout.",
    long = "`pattern` may be a plain string (substring match), a glob, or a regular expression.",
    example = "$t:wait_for \"# \"",
)]
struct WaitFor {
    #[description("the text (string, glob, or regular expression) to wait for.")]
    pattern: Patterns,
    #[default(Duration::seconds(5))]
    #[description("how long to wait before giving up.")]
    timeout: Duration,
}

fn wait_for(mut context: CommandContext) -> CrushResult<()> {
    let cfg: WaitFor =
        WaitFor::parse(context.remove_arguments(), &context.global_state.printer())?;
    let session = VtSession::get(&context.this.r#struct()?)?;
    session.wait_for(&cfg.pattern, cfg.timeout)?;
    context.output.send(Value::Empty)
}

#[signature(
    vt.session.contents,
    can_block = false,
    output = Known(ValueType::String),
    short = "The virtual terminal's current rendered screen, as plain text.",
)]
struct Contents {}

fn contents(mut context: CommandContext) -> CrushResult<()> {
    let session = VtSession::get(&context.this.r#struct()?)?;
    context.output.send(Value::from(session.contents()))
}

#[signature(
    vt.session.close,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Terminate this virtual terminal's command and release its resources.",
)]
struct Close {}

fn close(mut context: CommandContext) -> CrushResult<()> {
    let this = context.this.r#struct()?;
    match this.get("id") {
        Some(Value::Integer(id)) => {
            VtSession::close(id as i32);
            context.output.send(Value::Empty)
        }
        _ => crate::lang::errors::command_error("Invalid method id"),
    }
}
```

- [ ] **Step 7: Write `src/builtins/vt/create.rs`**

```rust
use crate::builtins::vt::methods::methods;
use crate::builtins::vt::session::VtSession;
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;
use std::path::PathBuf;

#[signature(
    vt.create,
    can_block = true,
    short = "Spawn a command attached to a fresh virtual terminal, for driving and inspecting a real interactive session from a script.",
    long = "Returns a struct with one method per operation (`write`, `wait_for`, `contents`,",
    long = "`close`, plus `pid`/`alive`) and a `control` struct of named control-character",
    long = "constants. Always call `close` once done, to release the underlying process.",
    example = "$t := (vt:create crush)",
    example = "$t:write \"echo hello\\n\"",
    example = "$t:wait_for \"hello\"",
    example = "$t:close",
)]
struct Create {
    #[description("the program to run, e.g. `crush`.")]
    command: PathBuf,
    #[unnamed()]
    #[description("arguments to pass to the program.")]
    arguments: Vec<String>,
    #[default(24)]
    #[description("the number of rows of the virtual terminal.")]
    rows: i128,
    #[default(80)]
    #[description("the number of columns of the virtual terminal.")]
    cols: i128,
}

fn create(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Create = Create::parse(context.remove_arguments(), &context.global_state.printer())?;
    let id = VtSession::create(&cfg.command, &cfg.arguments, cfg.rows as u16, cfg.cols as u16)?;

    let mut fields = vec![];
    for name in ["write", "wait_for", "contents", "close"] {
        fields.push((name, method_struct(id, name)?));
    }

    context.output.send(Value::Struct(Struct::new(fields, None)))
}

// `pub(super)` (not private) so Task 3 can reuse this when it extends the field list with
// `pid`/`alive`/`control`.
pub(super) fn method_struct(id: i32, name: &str) -> CrushResult<Value> {
    let cmd = match methods().get(name) {
        Some(cmd) => cmd.clone(),
        None => return command_error(format!("Unknown vt method `{}`", name)),
    };
    Ok(Value::Struct(Struct::new(
        vec![("id", Value::from(id)), ("__call__", Value::Command(cmd))],
        None,
    )))
}
```

- [ ] **Step 8: Write `src/builtins/vt/mod.rs`**

```rust
use crate::CrushResult;
use crate::builtins::vt::create::Create;
use crate::lang::state::scope::Scope;

mod control;
mod create;
mod methods;
mod session;

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "vt",
        "Virtual terminals, for driving and inspecting a real interactive session from a script",
        None,
        Box::new(move |env| {
            Create::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}

/// Called exactly once, unconditionally, near the end of `main.rs`'s `run()` -- see
/// src/builtins/vt/session.rs's `VtSession::close_all` doc comment for why.
pub fn close_all_sessions() {
    session::VtSession::close_all();
}
```

(`control.rs` itself is written in Task 3, which is also when `vt:control`/`$t:control` are
wired in; leave `mod control;` present but add the file as an empty stub for now so this
compiles: `touch src/builtins/vt/control.rs` with no content is enough for an empty private
module.)

- [ ] **Step 9: Wire the new module into `src/builtins/mod.rs`**

Change line 27 from `pub mod types;` region's neighborhood — add a new `pub mod vt;` module
declaration and a call in `declare()`. Concretely, in `src/builtins/mod.rs`:

```rust
mod term;
pub mod types;
mod users;
mod var;
pub mod vt;
mod sockets;
```

and:

```rust
    term::declare(root)?;
    types::declare(root)?;
    users::declare(root)?;
    var::declare(root)?;
    vt::declare(root)?;
    sockets::declare(root)?;
```

(`vt` is `pub mod`, matching the existing `pub mod types;` exception on the line above it —
needed so Task 4's `src/main.rs` change can call `crate::builtins::vt::close_all_sessions()`.)

- [ ] **Step 10: Build and run the smoke test**

```bash
cargo build && cargo test --test system test_vt_smoke -- --nocapture
```

Expected: PASS, output `ok`.

- [ ] **Step 11: Run it a few more times to rule out flakiness, then check for leaked processes**

```bash
for i in 1 2 3 4 5; do cargo test --test system test_vt_smoke || break; done
ps aux | grep -i '[c]at$'
```

Expected: 5/5 pass, and the `ps` check shows no leftover `cat` process (the test's own
`$t:close` call should already have reaped it; this is just confirming `Drop for VtSession`
actually works, since Task 4's exit hook hasn't been added yet and this is cheap to check
now while it's this one process's fault if it fails).

- [ ] **Step 12: Commit**

```bash
git add Cargo.toml src/lang/signature/patterns.rs src/builtins/mod.rs src/builtins/vt \
        tests/vt_smoke.crush tests/vt_smoke.crush.output
git commit -m "Add vt builtin: virtual-terminal sessions for interactive testing

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 3: `pid`/`alive` methods, and the `control` character constants

**Files:**
- Modify: `src/builtins/vt/methods.rs` (add `Pid`, `Alive`)
- Modify: `src/builtins/vt/create.rs` (add `pid`/`alive`/`control` to the returned struct)
- Write: `src/builtins/vt/control.rs` (replacing Task 2's empty stub)
- Test: `tests/vt_pid_alive_and_control.crush` + `.crush.output`

**Interfaces:**
- Consumes: `VtSession::pid`/`VtSession::alive` from Task 2; `create.rs`'s
  `method_struct(id, name)` helper from Task 2.
- Produces: `methods()` now also has `pid`/`alive` entries; `control::control_value() -> &'static Value`,
  consumed both by `create.rs` (per-session `$t:control`) and `mod.rs` (module-level `vt:control`).

- [ ] **Step 1: Write the test first**

`cat` has no prompt of its own to `wait_for`, so "did Ctrl-D actually end it" is checked by
polling `:alive` directly in a loop (with a hard iteration cap so a real bug makes the test
fail promptly instead of hanging) rather than waiting for any particular screen text:

```
# tests/vt_pid_alive_and_control.crush
$t := (vt:create cat)
assert ($t:alive)
assert (($t:pid) > 0)
$t:write $t:control:d
$tries := 0
while {($tries < 50) and ($t:alive)} {
    sleep $(duration:of milliseconds=100)
    $tries = $tries + 1
}
assert (not ($t:alive))
$t:close
echo "ok"
```

```
# tests/vt_pid_alive_and_control.crush.output
ok
```

- [ ] **Step 2: Run it to see it fail**

```bash
cargo build && cargo test --test system test_vt_pid_alive_and_control
```

Expected: FAIL — `pid`/`alive`/`control` don't exist on the returned struct yet.

- [ ] **Step 3: Add `Pid`/`Alive` to `src/builtins/vt/methods.rs`**

Add alongside the existing method structs/handlers, and register both in `methods()`:

```rust
#[signature(
    vt.session.pid,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "The process id of the command this virtual terminal is running.",
)]
struct Pid {}

fn pid(mut context: CommandContext) -> CrushResult<()> {
    let session = VtSession::get(&context.this.r#struct()?)?;
    context.output.send(Value::from(session.pid()))
}

#[signature(
    vt.session.alive,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "Whether the command this virtual terminal is running is still alive.",
)]
struct Alive {}

fn alive(mut context: CommandContext) -> CrushResult<()> {
    let session = VtSession::get(&context.this.r#struct()?)?;
    context.output.send(Value::Bool(session.alive()?))
}
```

and in `methods()`:

```rust
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        Write::declare_method(&mut res);
        WaitFor::declare_method(&mut res);
        Contents::declare_method(&mut res);
        Pid::declare_method(&mut res);
        Alive::declare_method(&mut res);
        Close::declare_method(&mut res);
        res
    })
```

- [ ] **Step 4: Write `src/builtins/vt/control.rs`**

```rust
use crate::lang::data::r#struct::Struct;
use crate::lang::value::Value;
use std::sync::OnceLock;

/// Named, one-character constants for control characters, so a script can write
/// `$t:write $t:control:z` (or `vt:control:z` at the module level) for Ctrl-Z instead of
/// a bare, hard-to-read `"\x1a"` literal. There is no separate "send a key" mechanism --
/// `write` takes these as perfectly ordinary text.
pub fn control_value() -> &'static Value {
    static CELL: OnceLock<Value> = OnceLock::new();
    CELL.get_or_init(|| {
        Value::Struct(Struct::new(
            vec![
                ("c", Value::from("\x03")), // Ctrl-C / SIGINT
                ("d", Value::from("\x04")), // Ctrl-D / EOF
                ("z", Value::from("\x1a")), // Ctrl-Z / SIGTSTP
            ],
            None,
        ))
    })
}
```

- [ ] **Step 5: Add `pid`/`alive`/`control` to the struct `create.rs` returns**

In `src/builtins/vt/create.rs`:

```rust
use crate::builtins::vt::control::control_value;
```

and in `fn create`:

```rust
    let mut fields = vec![];
    for name in ["write", "wait_for", "contents", "pid", "alive", "close"] {
        fields.push((name, method_struct(id, name)?));
    }
    fields.push(("control", control_value().clone()));

    context.output.send(Value::Struct(Struct::new(fields, None)))
```

- [ ] **Step 6: Expose `vt:control` at the module level too**

In `src/builtins/vt/mod.rs`:

```rust
use crate::builtins::vt::control::control_value;
...
        Box::new(move |env| {
            Create::declare(env)?;
            env.declare("control", control_value().clone())?;
            Ok(())
        }),
```

- [ ] **Step 7: Build and run the test**

```bash
cargo build && cargo test --test system test_vt_pid_alive_and_control -- --nocapture
```

Expected: PASS, output `ok`. If `assert`'s exact syntax above doesn't match this
codebase's real `assert` builtin signature, fix the test script to match it (check
`src/builtins/*.rs` for `assert`'s actual signature/arity) rather than guessing further —
this is exactly the kind of small mismatch Step 2's failing run should have already
surfaced.

- [ ] **Step 8: Commit**

```bash
git add src/builtins/vt tests/vt_pid_alive_and_control.crush tests/vt_pid_alive_and_control.crush.output
git commit -m "vt: add pid/alive methods and control-character constants

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 4: Clean up a `vt` session on process exit

**Files:**
- Modify: `src/main.rs:141-144`

**Interfaces:**
- Consumes: `crate::builtins::vt::close_all_sessions()` from Task 2.

- [ ] **Step 1: Reproduce the leak before the fix**

This check's whole point is inspecting OS process state *after* the crush process that
would run any `tests/*.crush` assertions has already exited, so it can't be expressed as a
golden-file stdout diff the way every other task's test is — drive it by hand instead,
with a throwaway script (not committed to the repo):

```bash
cargo build
echo '$t := (vt:create cat)
echo (string:format "{}" $t:pid)' > /tmp/vt_leak_check.crush
PID=$(./target/debug/crush /tmp/vt_leak_check.crush | tail -1)
sleep 1
ps -p "$PID" -o pid=,comm= || echo "already gone"
```

Expected (before the fix): `ps` still finds `$PID` as a running `cat` — confirming the leak
this task fixes. (If it's already gone, something else already reaps it; re-check by
removing Task 2's `Drop for VtSession` temporarily to be sure this reproduction is real
before trusting it, then restore it.)

- [ ] **Step 2: Add the `close_all_sessions()` call to `src/main.rs`**

In `src/main.rs`, the end of `run()` currently reads (around line 141-144):

```rust
    let status = global_state.exit_status().unwrap_or(0);
    global_state.threads().join(global_state.printer());
    drop(pretty_printer);
    drop(global_state);
```

Change it to:

```rust
    let status = global_state.exit_status().unwrap_or(0);
    global_state.threads().join(global_state.printer());
    // Runs unconditionally, regardless of Mode or whether the script called `exit` --
    // a vt session a script forgot to :close (or errored out before reaching) must not
    // leave its spawned process running past the end of the crush process that created
    // it.
    crate::builtins::vt::close_all_sessions();
    drop(pretty_printer);
    drop(global_state);
```

- [ ] **Step 3: Confirm the fix with the same manual repro**

```bash
cargo build
PID=$(./target/debug/crush /tmp/vt_leak_check.crush | tail -1)
sleep 1
ps -p "$PID" -o pid=,comm= || echo "already gone"
rm /tmp/vt_leak_check.crush
```

Expected: `already gone` — the `cat` process no longer outlives the crush process that
spawned it.

- [ ] **Step 4: Run the full suite**

```bash
cargo test
```

Expected: all pass, including the Task 2/3 `vt_*` tests (this change must not affect a
script that *does* call `:close` explicitly).

- [ ] **Step 5: Commit**

```bash
git add src/main.rs
git commit -m "vt: reap any still-open sessions on process exit

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 5: Expose the real binary under test to `.crush` scripts, and the first job-control test — Ctrl-Z suspends an external job

**Files:**
- Modify: `tests/system.rs` (inside the existing `start_test_servers` `#[ctor]`)
- Test: `tests/vt_ctrl_z_suspends_external_job.crush` + `.crush.output`

**Interfaces:**
- Consumes: `crush:env["CRUSH_BIN"]` (a new entry in the *existing* `crush:env` environment-variable
  mechanism; no new builtin), `vt:create`, `$t:write`, `$t:wait_for`, `$t:control:z`, `$t:close`.

- [ ] **Step 1: Write the test first**

```
# tests/vt_ctrl_z_suspends_external_job.crush
# Ctrl-Z on a running *external* job (`cmd sleep 100`) must suspend it and return control
# to the nested crush session's own prompt, with the job showing up as Paused.
$crush_bin := $(crush:env["CRUSH_BIN"])
$t := (vt:create $crush_bin)
$t:wait_for "# "
$t:write "cmd sleep 100\n"
$t:wait_for "sleep"
$t:write $t:control:z
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Paused"
$t:close
echo "ok"
```

```
# tests/vt_ctrl_z_suspends_external_job.crush.output
ok
```

- [ ] **Step 2: Run it to see it fail**

```bash
cargo build && cargo test --test system test_vt_ctrl_z_suspends_external_job
```

Expected: FAIL — `crush:env["CRUSH_BIN"]` isn't set yet, so `vt:create` gets an empty/invalid
program.

- [ ] **Step 3: Expose the real binary path via `CRUSH_BIN`**

In `tests/system.rs`'s existing `start_test_servers()` (the `#[ctor]` that already sets
`DNS_PORT`/`GRPC_PORT`/`SSHD_PORT` the same way), add one more line:

```rust
    unsafe {
        std::env::set_var("DNS_PORT", dns_port.to_string());
        std::env::set_var("GRPC_PORT", grpc_port.to_string());
        std::env::set_var("SSHD_PORT", ssh_port.to_string());
        // So a .crush test can spawn *the exact binary under test* inside a vt session
        // (vt:create $(crush:env["CRUSH_BIN"])) instead of relying on a `crush` found on
        // $PATH -- which during `cargo test` is not guaranteed to be this build at all.
        std::env::set_var("CRUSH_BIN", env!("CARGO_BIN_EXE_crush"));
    }
```

- [ ] **Step 4: Build and run**

```bash
cargo build && cargo test --test system test_vt_ctrl_z_suspends_external_job -- --nocapture
```

Expected: PASS, output `ok`. If it fails for a reason *other* than the `CRUSH_BIN` wiring
(e.g. a real job-control bug in how Ctrl-Z is delivered through this particular pty setup),
use the `superpowers:systematic-debugging` skill to find the actual root cause before
changing anything — this is precisely the kind of bug the original request asked to have
fixed if found, not worked around.

- [ ] **Step 5: Run it a few more times, and check nothing leaked**

```bash
for i in 1 2 3 4 5; do cargo test --test system test_vt_ctrl_z_suspends_external_job || break; done
ps aux | grep -i '[s]leep 100'
```

Expected: 5/5 pass, no leftover `sleep 100` process.

- [ ] **Step 6: Commit**

```bash
git add tests/system.rs tests/vt_ctrl_z_suspends_external_job.crush tests/vt_ctrl_z_suspends_external_job.crush.output
git commit -m "vt: expose CRUSH_BIN to tests; add Ctrl-Z/external job-control test

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 6: Ctrl-Z suspends an internal (builtin) job

**Files:**
- Test: `tests/vt_ctrl_z_suspends_internal_job.crush` + `.crush.output`

**Interfaces:** Consumes everything from Task 5; no new Rust code.

- [ ] **Step 1: Write the test**

Unlike the external-command test, a bare (no `&`) `sleep ...` prints nothing distinctive
while it runs and no new prompt appears until it's done or interrupted, so there is nothing
to `wait_for` between submitting it and sending Ctrl-Z -- a short, fixed settle delay is
used there instead:

```
# tests/vt_ctrl_z_suspends_internal_job.crush
$crush_bin := $(crush:env["CRUSH_BIN"])
$t := (vt:create $crush_bin)
$t:wait_for "# "
$t:write "sleep $(duration:of seconds=100)\n"
sleep $(duration:of milliseconds=200)
$t:write $t:control:z
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Paused"
$t:close
echo "ok"
```

```
# tests/vt_ctrl_z_suspends_internal_job.crush.output
ok
```

- [ ] **Step 2: Run it**

```bash
cargo build && cargo test --test system test_vt_ctrl_z_suspends_internal_job -- --nocapture
```

Expected: PASS, output `ok`. If Ctrl-Z doesn't actually suspend an *internal* job the same
way it suspends an external one (e.g. because job-control signal delivery was only ever
exercised against real child processes before this), use `superpowers:systematic-debugging`
on crush's own job-control code (`src/builtins/control/cmd.rs`, `src/lang/job_control.rs`)
to find and fix the real root cause — this is exactly the kind of gap the original request
anticipated ("test that you can spawn multiple external and internal commands").

- [ ] **Step 3: Commit**

```bash
git add tests/vt_ctrl_z_suspends_internal_job.crush tests/vt_ctrl_z_suspends_internal_job.crush.output
git commit -m "vt: add Ctrl-Z/internal-job job-control test

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 7: `fg` moves control between two backgrounded jobs (one external, one internal)

**Files:**
- Test: `tests/vt_fg_moves_control_between_jobs.crush` + `.crush.output`

**Interfaces:** Consumes everything from Task 6; no new Rust code.

- [ ] **Step 1: Write the test**

```
# tests/vt_fg_moves_control_between_jobs.crush
# Start one external and one internal job in the background, Ctrl-Z/fg between them, and
# confirm control genuinely moves -- each job prints its own distinct marker right before
# it would finish, so "did the right one actually run to completion" is a plain substring
# check, not a guess based on timing.
$crush_bin := $(crush:env["CRUSH_BIN"])
$t := (vt:create $crush_bin)
$t:wait_for "# "

$t:write "cmd sh -c 'sleep 1; echo EXTERNAL_DONE' &\n"
$t:wait_for "# "
$t:write "sleep $(duration:of seconds=1); echo INTERNAL_DONE &\n"
$t:wait_for "# "

$t:write "fg\n"
$t:wait_for "EXTERNAL_DONE"
$t:wait_for "# "

$t:write "fg\n"
$t:wait_for "INTERNAL_DONE"
$t:wait_for "# "

$t:close
echo "ok"
```

```
# tests/vt_fg_moves_control_between_jobs.crush.output
ok
```

(`fg` with no job id operates on the most recently backgrounded/paused job, per `Fg.job:
Option<usize>` in `src/builtins/control/mod.rs` — calling it twice in a row should bring
each of the two backgrounded jobs to the foreground in turn.)

- [ ] **Step 2: Run it**

```bash
cargo build && cargo test --test system test_vt_fg_moves_control_between_jobs -- --nocapture
```

Expected: PASS, output `ok`. If bare `fg` doesn't pick the jobs in the expected order, or
if the internal job's trailing `echo` doesn't run as part of the same backgrounded job
(check whether `;`-sequenced commands followed by `&` background the whole sequence or
just the last part — adjust the script to make that unambiguous, e.g. wrap it in `{...}
&`, if needed) — use `superpowers:systematic-debugging` to find out which, then fix either
the test or, if it's a real `fg`/job-sequencing bug, the root cause in
`src/builtins/control/mod.rs`.

- [ ] **Step 3: Run it a few more times, check nothing leaked**

```bash
for i in 1 2 3 4 5; do cargo test --test system test_vt_fg_moves_control_between_jobs || break; done
ps aux | grep -i '[s]leep 1'
```

- [ ] **Step 4: Commit**

```bash
git add tests/vt_fg_moves_control_between_jobs.crush tests/vt_fg_moves_control_between_jobs.crush.output
git commit -m "vt: add fg-moves-control-between-jobs test

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 8: `bg` resumes a paused job without foregrounding it

**Files:**
- Test: `tests/vt_bg_resumes_without_foregrounding.crush` + `.crush.output`

**Interfaces:** Consumes everything from Task 7; no new Rust code.

- [ ] **Step 1: Write the test**

```
# tests/vt_bg_resumes_without_foregrounding.crush
# Pause a job with Ctrl-Z, then `bg` it, and confirm it's running again (status flips back
# from Paused to Running) *without* the prompt being occupied -- i.e. control stays with
# the shell the whole time, unlike `fg`.
$crush_bin := $(crush:env["CRUSH_BIN"])
$t := (vt:create $crush_bin)
$t:wait_for "# "

$t:write "cmd sleep 100\n"
$t:wait_for "sleep"
$t:write $t:control:z
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Paused"

$t:write "bg 1\n"
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Running"

$t:close
echo "ok"
```

```
# tests/vt_bg_resumes_without_foregrounding.crush.output
ok
```

(Job id `1` assumes this is the first job started in this session — true here since it's
the only job this script ever starts. If that assumption turns out wrong when actually run
against the real `bg` command's id semantics, fix the test to look the id up explicitly
instead of hardcoding it, the same way Task 7 could if it needed to, rather than guessing a
different hardcoded number.)

- [ ] **Step 2: Run it**

```bash
cargo build && cargo test --test system test_vt_bg_resumes_without_foregrounding -- --nocapture
```

Expected: PASS, output `ok`. Debug any real `bg` issue via `superpowers:systematic-debugging`
before changing anything, same as earlier tasks.

- [ ] **Step 3: Run it a few more times, check nothing leaked**

```bash
for i in 1 2 3 4 5; do cargo test --test system test_vt_bg_resumes_without_foregrounding || break; done
ps aux | grep -i '[s]leep 100'
```

- [ ] **Step 4: Commit**

```bash
git add tests/vt_bg_resumes_without_foregrounding.crush tests/vt_bg_resumes_without_foregrounding.crush.output
git commit -m "vt: add bg-resumes-without-foregrounding test

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 9: `terminate` and `host:signal` both work on an external job

**Files:**
- Test: `tests/vt_terminate_and_signal_external_job.crush` + `.crush.output`

**Interfaces:** Consumes everything from Task 8; no new Rust code.

- [ ] **Step 1: Write the test**

```
# tests/vt_terminate_and_signal_external_job.crush
# Both `terminate <job id>` and `host:signal <pid>` must be able to kill a running
# *external* job -- the former through crush's own job-control plumbing, the latter by
# sending a real OS signal directly to the process, bypassing that plumbing entirely.
$crush_bin := $(crush:env["CRUSH_BIN"])
$t := (vt:create $crush_bin)
$t:wait_for "# "

$t:write "cmd sleep 100 &\n"
$t:wait_for "# "
$t:write "terminate 1\n"
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Terminated"

$t:write "cmd sleep 101 &\n"
$t:wait_for "# "
$t:write "($(host:procs | where {($name == \"sleep\")} | select pid):rows)[0][0] | host:signal signal=SIGKILL\n"
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Terminated"

$t:close
echo "ok"
```

```
# tests/vt_terminate_and_signal_external_job.crush.output
ok
```

(The `host:procs | where {...} | select pid` pipeline looks up the real OS pid of the
`sleep` process from *inside* the nested session, so `host:signal` is given a real pid, not
a job id — exercising the "bypasses crush's own job control" path the original request
specifically asked for.)

- [ ] **Step 2: Run it**

```bash
cargo build && cargo test --test system test_vt_terminate_and_signal_external_job -- --nocapture
```

Expected: PASS, output `ok`. If the `host:procs`/`host:signal` one-liner's exact syntax
doesn't match this codebase's real signatures (check `src/builtins/host.rs` for `Procs`'s
and `Signal`'s actual field names/types), fix the test script to match rather than guessing
further — and if `jobs` genuinely never shows `Terminated` after a `host:signal`-killed
process (as opposed to crush's own `terminate`), that's a real gap in how crush reaps
externally-killed jobs; use `superpowers:systematic-debugging` to find and fix the root
cause, per the original request.

- [ ] **Step 3: Run it a few more times, check nothing leaked**

```bash
for i in 1 2 3 4 5; do cargo test --test system test_vt_terminate_and_signal_external_job || break; done
ps aux | grep -i '[s]leep 10[01]'
```

- [ ] **Step 4: Commit**

```bash
git add tests/vt_terminate_and_signal_external_job.crush tests/vt_terminate_and_signal_external_job.crush.output
git commit -m "vt: add terminate + host:signal external-job test

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 10: `terminate` works on an internal job

**Files:**
- Test: `tests/vt_terminate_internal_job.crush` + `.crush.output`

**Interfaces:** Consumes everything from Task 9; no new Rust code.

- [ ] **Step 1: Write the test**

```
# tests/vt_terminate_internal_job.crush
# `terminate` must stop an *internal* job's underlying thread, not just flip a status flag
# -- confirmed here by terminating a backgrounded builtin `sleep` long before it would
# finish on its own, then confirming it never prints the marker that would follow it.
$crush_bin := $(crush:env["CRUSH_BIN"])
$t := (vt:create $crush_bin)
$t:wait_for "# "

$t:write "sleep $(duration:of seconds=100); echo SHOULD_NOT_APPEAR &\n"
$t:wait_for "# "
$t:write "terminate 1\n"
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Terminated"

sleep $(duration:of seconds=1)
$t:write "echo MARKER\n"
$t:wait_for "MARKER"
assert (not ($t:contents | string:contains "SHOULD_NOT_APPEAR"))

$t:close
echo "ok"
```

```
# tests/vt_terminate_internal_job.crush.output
ok
```

If `assert`'s/`string:contains`'s exact call shape above doesn't match this codebase's real
signatures, fix the script to match the real ones (check `src/builtins/*.rs`) rather than
guessing further.

- [ ] **Step 2: Run it**

```bash
cargo build && cargo test --test system test_vt_terminate_internal_job -- --nocapture
```

Expected: PASS, output `ok`. If `terminate` flips the job's status to `Terminated` but the
underlying `sleep`/`echo` sequence still runs to completion in the background regardless
(i.e. `SHOULD_NOT_APPEAR` does show up), that is exactly the kind of real bug the original
request asked to have found and fixed — use `superpowers:systematic-debugging` starting
from `GlobalState::terminate`/`JobControlData::terminate` (`src/lang/state/global_state.rs`,
`src/lang/state/handles.rs`) and the closure-evaluation code that's supposed to observe a
termination request (`src/lang/command/closure.rs`) to find the real root cause before
changing anything.

- [ ] **Step 3: Run it a few more times**

```bash
for i in 1 2 3 4 5; do cargo test --test system test_vt_terminate_internal_job || break; done
```

- [ ] **Step 4: Commit**

```bash
git add tests/vt_terminate_internal_job.crush tests/vt_terminate_internal_job.crush.output
git commit -m "vt: add terminate internal-job test

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

---

### Task 11: Full-suite stability pass

**Files:** None (verification only).

**Interfaces:** None.

- [ ] **Step 1: Run the entire test suite**

```bash
cargo build && cargo test
```

Expected: everything passes, including every pre-existing test (nothing in this plan should
have touched behavior outside the `vt` namespace and the one `main.rs`/`cmd.rs`-adjacent fix
from a debugging step above, if any was needed).

- [ ] **Step 2: Run every new `vt_*` test several times, sequentially**

```bash
for f in tests/vt_*.crush; do
    name="test_$(basename "$f" .crush)"
    for i in 1 2 3 4 5; do
        cargo test --test system "$name" || { echo "FLAKY: $name run $i"; break; }
    done
done
```

Expected: 5/5 for every one of them. Investigate (via `superpowers:systematic-debugging`,
not a retry-until-it-passes loop) anything that isn't.

- [ ] **Step 3: Confirm no leaked processes across the whole battery**

```bash
ps aux | grep -iE '[s]leep (1|10[01])|[c]at$'
```

Expected: empty.

- [ ] **Step 4: Regenerate the builtin docs**

Per this project's own `CLAUDE.md`: `docs/builtins.html` must be regenerated after adding
builtins.

```bash
cargo build && ./target/debug/crush generate_docs.crush > docs/builtins.html
git add docs/builtins.html
git commit -m "Regenerate docs/builtins.html for the new vt namespace

Co-Authored-By: Claude Sonnet 5 <noreply@anthropic.com>"
```

- [ ] **Step 5: Hand off**

No further action — this is the last task. Report the final state (tests added, any real
crush bugs found and fixed along the way, and their task numbers) back for review.
