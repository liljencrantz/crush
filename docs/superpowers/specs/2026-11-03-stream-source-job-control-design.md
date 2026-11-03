# Stream-source job control audit — design

## Problem

Crush's job control (`terminate`, Ctrl-Z/`pause`, `bg`/`resume`, `fg`) only actually
stops/suspends a builtin's own execution if that builtin explicitly cooperates — registers
a `JobController` and checks it. `ThreadStore::spawn` always registers a generic wrapper
controller too, but that wrapper can't interrupt code it didn't write: on `Pause` it just
leaves the thread registered (doc comment on `ThreadStore::join_one`), and on `Terminate`
it just stops *tracking* the thread (`StreamControlMessage::Terminate => {}` in the same
function) — the thread itself keeps running, orphaned, until it finishes on its own.
`control.timeout`'s own doc comment says this plainly: "most builtins don't [cooperate],
and will keep running in the background regardless."

Recent work (commits `ea7762f2`..`97e790e5`) fixed this gap for `cmd`, `schedule`, and
`stream:tee`. This spec extends the same audit to **source commands**: builtins whose
signature produces a stream (`table_input_stream`/`BinaryInputStream` output) and consumes
no stream input at all — i.e. they *generate* rows rather than transform an existing
stream. These are the commands most likely to run long enough for job control to matter
(an infinite generator, a slow recursive scan, a `follow`-mode watcher), and a previous
pattern (`src/builtins/fs/watch.rs`) already shows the correct cooperative shape.

## Command inventory

Found via `rg "output = Known\(ValueType::(table_input_stream|TableInputStream|
BinaryInputStream)"` across `src/builtins`, filtered to `can_block = true` (a
`can_block = false` command never leaves the calling thread, so there is no window for a
signal to land in) and to signatures with no stream-input parameter.

| Command | Shape | Job control today | Action |
|---|---|---|---|
| `random:float_stream` | **infinite** tight loop | none at all | **fix** (mirror `fs:watch`) |
| `random:integer_stream` | **infinite** tight loop | none at all | **fix** (mirror `fs:watch`) |
| `fs:usage` | recursive dir walk, can run for minutes on a big tree | none at all | **fix** |
| `systemd:journal --follow` | blocks indefinitely on `await_next_entry(None)` (Linux-only) | none at all | **fix, best-effort** (can't build/run this module on this macOS dev machine — Linux-gated via `#[cfg(target_os = "linux")]`) |
| `fs:watch` | blocks on OS fs-change notifications, can run indefinitely | already correct (`select!` + registered controller) | regression test only |
| `remote:pexec` | parallel SSH calls, bounded by host count/timeouts | already correct (each worker registers its own controller via `run_remote`) | regression test only |
| a `file` value's `:remove recursive=$true` method (`types.file.remove`) | recursive delete, can be slow on a big tree | none | test; fix only if the test shows it doesn't stop early (see Task 8) |
| `sockets:tcp`, `sockets:udp`, `host:battery`, `host:procs`, `host:threads`, `groups:list`, `users:list`, `remote:identity`, `remote:host:list`, `fs:stat`, `fs:mounts`, `crush:history` | one-shot snapshot, returns in well under a second | n/a (nothing to interrupt) | smoke test only, for completeness |

Excluded as out of scope: every `can_block = false` command (no thread, no interruptible
window); every command that *consumes* stream input (e.g. `io:lines:from`, every codec's
`:from`, `stream:*`, and — despite its `table_input_stream` output — `types.members`
(`$value | members`), which reads `context.input_stream()` and so is a reflection/transform
over an existing stream, not a source) — those already share the pipeline's existing
stream-control plumbing; `var:list` and `control:jobs`, which are `can_block = false`.

## The existing pty test idiom, and what makes it cumbersome

Every existing `tests/pty_*.crush` test repeats the same three things, confirmed directly
against `src/builtins/pty/session.rs`:

1. `$t:wait_for "# "` right after a `$t:write "...\n"` is satisfied by the *prompt already
   on screen before the write landed* — a real pty's line discipline echoes the typed line
   right after the still-visible old prompt, on the same line, so `wait_for` can return
   true before the command was even submitted. Every test works around this with a blind
   `sleep $(duration:of milliseconds=300)` (or longer) after almost every interaction — see
   `pty_fg_moves_control_between_jobs.crush`'s own header comment for a worked example of
   the race this papers over.
2. Getting the pid of a process the nested session just spawned currently means writing
   `$my_pid := $(crush:pid)` *into* the nested session and scraping it back off the
   rendered screen, even though the pty struct already exposes the nested shell's own pid
   directly as `$t:pid` (`PtySession::pid`, already returned by `pty:create`) — no round
   trip needed.
3. Every test re-derives the same "spawn a nested crush, wait for its first prompt" and
   "read a job's status out of `jobs`" boilerplate from scratch.

## Design

### `pty` builtin: a quiescence primitive

Add `PtySession::wait_quiet(&self, quiet: Duration, timeout: Duration) -> CrushResult<()>`
in `session.rs`: polls the same way `wait_for` already does (`rx.lock().recv_timeout
(POLL_INTERVAL)`, same parser-before-rx lock ordering), but returns once no new bytes have
arrived for `quiet`, instead of once a pattern matches. This directly replaces "wait for a
fixed string, then sleep a fixed guess" with "wait until the terminal actually stops
producing output" — adaptive (keeps waiting if the command is still genuinely producing
output) where a fixed sleep is not, and correct for the "nothing distinctive prints while
it runs" case (e.g. a bare `sleep`) that today has *no* synchronization signal at all other
than the blind delay.

Expose two new session methods (`methods.rs`, registered in `create.rs`'s method list
alongside the existing six):

- `wait_quiet(quiet: Duration = 100ms, timeout: Duration = 5s)` — the primitive above,
  directly callable when a test needs to wait for quiet without also sending a line (e.g.
  right after a control character).
- `run(text: String, quiet: Duration = 100ms, timeout: Duration = 5s)` — sugar for
  `write_raw(text + "\n")` then `wait_quiet(quiet, timeout)`. The common case: submit one
  command line and don't do anything else until the session has actually processed it.

`wait_for` itself is unchanged — pattern matching on screen content is still exactly right
for "did this specific text ever appear" (e.g. a job's status, a side-effect marker).

### Shared test helper library

A new `tests/pty_test_helpers.crush`, loaded with `source "tests/pty_test_helpers.crush"`,
providing:

- a helper that spawns a nested crush session and waits for its first prompt, returning the
  session struct (replaces the repeated `$crush_bin := ...; $t := $(pty:create ...);
  $t:wait_for "# "` preamble in every test);
- a helper that finds the pid of a child process of `$t:pid` matching a name pattern via
  `host:procs`, asserting exactly one match (replaces the `$my_pid := $(crush:pid)`
  round-trip described above);
- a helper that reads one job's `status` column back out of the nested session's own
  `jobs` table (still a round trip through the rendered screen — there is no other channel
  — but written once instead of once per test).

Existing `tests/pty_*.crush` tests are left as they are; only new tests in this plan use
the shared helpers and the new `run`/`wait_quiet` methods.

## Fix shape (shared across random streams / fs:usage / systemd:journal)

Mirror `fs:watch`'s pattern: register a `ChannelBasedController` via
`context.command_handle().register(...)`, then check the paired receiver on every loop
iteration (or, for `fs:usage`'s recursive walk, on every file/directory visited):
`Terminate` → return `crate::lang::errors::terminate()`; `Pause` → block in an inner loop
on `control_receiver.recv()` until `Resume`/`ResumeForeground` (or `Terminate`, same
return); `Resume`/`ResumeForeground` arriving outside a pause → no-op. `random:*_stream`
and `fs:usage`'s recursive walk have no I/O event to `select!` against, so they use a
non-blocking `control_receiver.try_recv()` each iteration instead of `fs:watch`'s
`select!`. `systemd:journal`'s `--follow` path already calls `journal.await_next_entry
(Some(d))`/`await_next_entry(None)`; change the `follow` branch to pass a short `Some(d)`
poll interval instead of `None` so the same `try_recv` check runs every time that call
returns with nothing new, instead of blocking forever inside the systemd library call.
