# `vt` — a virtual-terminal builtin for interactive testing

## Summary

Add a new `vt` builtin namespace that spawns a PTY-backed child process and
exposes it to crush scripts as a live, write-and-poll handle: `vt:create`,
`$t:write`, `$t:wait_for`, `$t:contents`, `$t:pid`, `$t:alive`, `$t:close`,
plus a small `control` struct of named control-character constants. This
lets interactive behavior (job control, `fg`/`bg`, signal handling, anything
that needs a real tty) be tested as plain `tests/*.crush` +
`tests/*.crush.output` golden-file pairs — the same auto-discovered,
no-Rust-code-needed convention every other test in this repo already uses —
instead of a bespoke Rust test harness with its own script format.

This supersedes the in-progress, uncommitted Rust PTY harness
(`tests/interactive_harness.rs`, `tests/interactive.rs`,
`tests/interactive/*.itest`, the `test_finder` generalization, and the
`vt100`/`[[test]] interactive` additions to `Cargo.toml`). That work proved
out the hard parts (PTY spawning, raw-mode/startup-race handling, terminal
emulation via `vt100`, process-tree cleanup) and those techniques carry over
directly into `VtSession` below, but the harness itself — and its `.itest`
format — is dead weight once `vt` exists, and will be removed as part of
this work.

## Motivation

The job-control system (Ctrl-Z, `fg`, `bg`, `terminate`, `host:signal`,
foreground/background process-group handoff) can only be exercised through
a real controlling terminal — there is no way to test it by piping
plain stdin/stdout into the `crush` binary the way `tests/system.rs` does
for everything else. Some kind of PTY is unavoidable. The question is only
which side of the Rust/crush boundary drives it from.

Driving it from crush scripts (rather than from a new, parallel Rust test
harness) means:
- Tests are ordinary `tests/*.crush` / `tests/*.crush.output` pairs,
  auto-discovered by the existing `test_finder!()` macro — no new test
  format, no new Rust test-runner code to maintain.
- A test can be prototyped interactively in the REPL (`vt:create`, poke at
  it, `:contents`) instead of edit-recompile-rerun.
- The "don't match output verbatim, match on specific text or file
  side-effects" requirement is just normal crush scripting (`:like`,
  `string:contains`, `wait-for`-style polling, `try`/`catch`) rather than a
  purpose-built assertion DSL.

## Non-goals

- `vt` is not a general terminal-multiplexing or scripting tool for
  end users; it exists to make interactive behavior testable. Nothing
  about the design should discourage trimming it down later if it turns
  out to be test-only infrastructure.
- No attempt to make `vt` sessions serializable (`pup:to`, `remote:exec`) —
  see "Error handling" below.
- No new `ValueType`. See "Rejected alternative" below.

## Design

### Resource model: a private registry, exactly like `grpc`

`src/builtins/grpc/client.rs` already solves "a live, non-`Clone`, mutable
native resource that a script holds a lightweight handle to": a private
`static SESSIONS: LazyLock<RwLock<HashMap<i32, Option<T>>>>` plus an
`AtomicI32` id counter is the only place the real resource lives
(`GrpcClient::register`/`close`/`get`/`get_from_id`, `client.rs:32-110`).
Scripts only ever hold a plain integer id, embedded in a `Struct`. `vt`
reuses this exact shape:

```rust
// src/builtins/vt/session.rs
static SESSIONS: LazyLock<RwLock<HashMap<i32, Option<VtSession>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
static NEXT_ID: AtomicI32 = AtomicI32::new(1);

pub struct VtSession {
    master_fd: OwnedFd,              // pty master; writes go straight to the fd,
                                      // no locking needed (write(2) is thread-safe)
    child: Mutex<Child>,              // try_wait() needs &mut
    rx: Receiver<Vec<u8>>,            // crossbeam Receiver::recv* takes &self
    parser: Mutex<vt100::Parser>,     // mutated by every drain
}

impl VtSession {
    pub fn register(session: VtSession) -> i32 { ... }
    pub fn close(id: i32) { ... }               // sets slot to None; Drop kills
                                                  // the process tree (reused from
                                                  // the old harness's sysinfo-based
                                                  // cleanup)
    pub fn get(s: &Struct) -> CrushResult<MappedRwLockReadGuard<'static, VtSession>> { ... }
    pub fn get_from_id(id: i32) -> CrushResult<MappedRwLockReadGuard<'static, VtSession>> { ... }
    pub fn close_all() { ... }                  // see "Process-exit cleanup" below

    pub fn write_raw(&self, text: &str) -> CrushResult<()> { ... }
    pub fn drain_and_render(&self) -> CrushResult<String> { ... }
    pub fn wait_for(&self, pattern: &Patterns, timeout: Duration) -> CrushResult<()> { ... }
    pub fn pid(&self) -> i32 { ... }
    pub fn alive(&self) -> CrushResult<bool> { ... }
}
```

Spawning (`openpty`, `pre_exec` with `setsid` + `TIOCSCTTY`, isolating
`XDG_CONFIG_HOME` to a scratch dir so a developer's own `~/.config/crush`
never leaks into a test, the reader thread feeding raw bytes into a
channel) is lifted close to verbatim from `tests/interactive_harness.rs`'s
`Session::spawn`.

### Rejected alternative: a new `ValueType::Vt`

An earlier version of this proposal added a brand-new `ValueType::Vt`
holding `Arc<Mutex<VtSession>>` directly, dispatched generically the way
`string`/`list`/`table_input_stream` methods are. Rejected because it has
no precedent for a *mutable OS resource* in this codebase (the one existing
resource-wrapping type, `BinaryInputStream`, is a read-only `Box<dyn
BinaryReader>` cloned by duplicating the fd, not shared mutable state), and
because the registry approach above is simpler, already proven out by
`grpc`, and needs no core type-system changes at all.

### The struct `vt:create` returns

Mirroring `grpc:connect` (`src/builtins/grpc/connect.rs:98-166`): the
top-level struct has one field per method, and *each method is its own
tiny sub-struct* of `{id, __call__}`, where `__call__` is a `Value::Command`
built via `<dyn CrushCommand>::command(...)`. Calling `$t:write ...` binds
that inner `{id, __call__}` sub-struct as `$this`; the handler function
recovers the session via `VtSession::get(&context.this.r#struct()?)`.

```
Struct {
  write:    Struct { id, __call__: Command(vt_write) },
  wait_for: Struct { id, __call__: Command(vt_wait_for) },
  contents: Struct { id, __call__: Command(vt_contents) },
  pid:      Struct { id, __call__: Command(vt_pid) },
  alive:    Struct { id, __call__: Command(vt_alive) },
  close:    Struct { id, __call__: Command(vt_close) },
  control:  Struct { c: "\x03", z: "\x1a", d: "\x04", ... }   // no id; shared constant
}
```

Each method with real arguments (`write`, `wait_for`) is declared with the
normal `#[signature(...)]` macro — the same macro `grpc:connect`'s own
`Connect` struct uses — purely for argument parsing, help text, and tab
completion, via its generated `declare_method()` (the same mechanism
`table_input_stream.rs`'s bundled `write`/`close` methods already use, see
`methods()` at `table_input_stream.rs:18-28`). `vt:create` builds one static
`OrderedMap<String, Command>` this way and clones the relevant `Command`
into each per-method sub-struct's `__call__` field.

### Methods

- **`vt:create`** — `#[signature(vt.create, can_block = true, ...)]`.
  Fields: `command: Vec<String>` (unnamed, the argv to spawn — a single
  string is also accepted and split, or require a list; TBD during
  implementation), `rows: i128` (default 24), `cols: i128` (default 80).
  Spawns the PTY + child, registers a `VtSession`, builds and returns the
  struct above.
- **`write`** — `#[signature(vt.write, ...)]`. One field: `text: String`
  (unnamed). Writes the literal bytes to the pty master. No implicit
  newline, no key-name translation — a control character is just written
  as itself (see "Control characters" below).
- **`wait_for`** — `#[signature(vt.wait_for, ...)]`. Fields: `pattern:
  Patterns` (unnamed — a plain string is an exact-substring match, matching
  how `grpc:connect`'s `service: Patterns` field already works via
  `Patterns::test`), `timeout: Duration` (default `Duration::seconds(5)`).
  Polls the rendered screen (draining the reader channel into the
  `vt100::Parser` each iteration) until `pattern.test(&contents)` is true,
  or returns `Err` with the current (trimmed) screen contents attached to
  the message once `timeout` elapses — this keeps the existing
  `try { $t:wait_for ... } catch { |$e| ... }` idiom available for tests
  that need to assert *why* something didn't appear.
- **`contents`** — `#[signature(vt.contents, ...)]`, no arguments. Drains
  and returns the current rendered screen as a trimmed plain string (ports
  `screen_contents()` from the old harness).
- **`pid`** — returns the child's pid as an integer (for, e.g.,
  `host:procs | where {($pid == ...)}` lookups from the test script).
- **`alive`** — returns a bool: has the child exited.
- **`close`** — tears the session down explicitly (kills the process tree,
  closes the pty, removes the registry entry). Scripts are expected to call
  this when done, exactly as `grpc:connect`'s docs already tell users to
  call `$conn:close`.

### Control characters

No `:key` method. `write` takes literal text; a control character is just
its actual byte (Ctrl-Z is `"\x1a"`, Ctrl-C is `"\x03"`, etc.). For
readability, a `control` struct of named one-character constants is
exposed — tentatively both off the module (`vt:control:z`) and off every
session (`$t:control:z`), sharing the same static `Struct` value, since it
doesn't depend on the session at all. Which of the two (or both) reads
better in practice is worth settling during implementation/early test
writing rather than deciding blind now.

### Process-exit cleanup

Unlike the discarded `ValueType` design, nothing here automatically drops
a `VtSession` when a script stops referencing it — the registry entry (and
therefore the child process) lives until `:close` is called, same as a
`grpc` connection today. A test that errors out mid-script (e.g. a
`wait_for` timeout propagating as an uncaught error) or simply forgets
`:close` would otherwise leak a real child process past the end of that
test run.

`control:exit`'s `force` path (`src/builtins/control/mod.rs:575-599`)
already has exactly this kind of "tear down leftover state before the
process exits" responsibility for background jobs. Add a
`VtSession::close_all()` call there (and/or at whatever point
`tests/system.rs`'s own driver tears down the `crush` subprocess after each
golden test) so a forgotten `:close` can't leak a process past the test
run that created it. It's possible the OS already mostly handles this on
its own — closing the pty master typically delivers `SIGHUP` to the
session leader reading the slave end, since the child is `setsid`'d onto
that tty — but the explicit sweep is cheap insurance and shouldn't be
skipped on the assumption that it does.

### File layout

```
src/builtins/vt/
  mod.rs        — declare(root), wires `create`/`control` into the `vt` namespace
  session.rs    — VtSession, the registry, spawn logic (ported from the old harness)
  methods.rs     — the #[signature(...)] structs + handler fns for write/wait_for/
                    contents/pid/alive/close, and the static OrderedMap<String, Command>
                    `vt:create` draws __call__ values from
```

`src/builtins/mod.rs::declare()` gets one new line, `vt::declare(root)?;`,
alongside the existing `grpc::declare(root)?;` etc.

### Error handling

- `VtSession::get`/`get_from_id` return the same "unknown id, did you
  already close this?" style error `GrpcClient::get_from_id` does
  (`client.rs:104-109`) when a script holds onto a sub-struct after
  calling `:close`.
- `vt` values are never reachable from `pup:to`/`remote:exec` in the first
  place (they're plain `Struct`s of integers/commands/strings, same as a
  `grpc` connection), so no special serialization handling is needed beyond
  what already exists for a `Struct` containing a `Command`.

## Testing plan

Every scenario from the original request becomes a `tests/vt_*.crush` +
`.crush.output` pair, e.g.:

```
# tests/vt_ctrl_z_external_returns_control.crush
$t := (vt:create command=["crush"])
$t:wait_for "# "
$t:write "cmd sleep 100\n"
$t:wait_for "sleep"
$t:write $t:control:z
$t:wait_for "# "
$t:write "jobs\n"
$t:wait_for "Paused"
echo "ok"
$t:close
```
```
# tests/vt_ctrl_z_external_returns_control.crush.output
ok
```

Planned coverage, mirroring the original request:
- Ctrl-Z suspends an external (`cmd sleep ...`) job and returns control to
  the prompt.
- Ctrl-Z suspends an internal (builtin `sleep`) job the same way.
- Multiple jobs (mix of internal/external), moving control between them
  with Ctrl-Z and `fg`.
- `bg` resumes a paused job without foregrounding it.
- `terminate` and `host:signal` both work on an external job.
- `terminate` works on an internal job.

Plus a couple of harness-smoke-test equivalents of what
`tests/interactive_harness.rs` already validated (plain command + output,
plain command + file side effect) as `vt` sanity checks, now just ordinary
`.crush` tests rather than a separate smoke-test category.

As with the earlier work, any real crush bugs this surfaces get fixed
before the test suite is considered done, per the original request ("If
you find bugs, try to fix them").

## Migration / cleanup

Delete `tests/interactive_harness.rs`, `tests/interactive.rs`,
`tests/interactive/*.itest`, and revert the `vt100` dev-dependency and the
`[[test]] name = "interactive"` entry in `Cargo.toml`. Keep the
`test_finder` generalization (`test_finder/src/lib.rs`'s `(dir, ext,
runner)` parameterization) only if something in the new plan still needs
it — plain `tests/vt_*.crush` files are picked up by the *existing*
`test_finder!()` call in `tests/system.rs` with no changes, so the
generalization is likely no longer needed either and can be reverted back
to the original hardcoded macro unless a reason to keep it turns up during
implementation.
