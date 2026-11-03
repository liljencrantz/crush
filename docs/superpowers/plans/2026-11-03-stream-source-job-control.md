# Stream-Source Job Control Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Audit every builtin that generates a stream without consuming one (table/binary
stream *sources*), give every one of them a `tests/pty_*.crush` job-control test (bg/fg,
Ctrl-Z, Ctrl-C/terminate), fix the real job-control gaps the audit found, and add a
quiescence primitive to the `pty` builtin so those tests (and future ones) are far less
boilerplate-heavy to write.

**Architecture:** Extend `src/builtins/pty` with a `wait_quiet`/`run` primitive that waits
for the pty's rendered screen to stop changing instead of matching a fixed string plus a
blind sleep. Add a small sourced crush helper library (`tests/pty_test_helpers.crush`) on
top of it. Fix the source commands found to ignore job control entirely
(`random:float_stream`, `random:integer_stream`, `fs:usage`, `systemd:journal --follow`)
by giving each the same cooperative-controller shape `src/builtins/fs/watch.rs` already
uses correctly. Write one small `pty_*_job_control.crush` test per remaining source
command for completeness.

**Tech Stack:** Rust (`crossbeam::channel`, the existing `JobControl`/`ChannelBasedController`
machinery in `src/lang/job_control.rs`), the project's `signature` macro, plain
`tests/*.crush` golden tests.

**Spec:** `docs/superpowers/specs/2026-11-03-stream-source-job-control-design.md`

## Global Constraints

- New tests are plain `tests/*.crush` (+ `.crush.output` where output is deterministic) —
  no new Rust assertions in `tests/system.rs`.
- `cargo build` must be rerun before `cargo test` picks up any Rust change in this plan.
- Regenerate `docs/builtins.html` (`cargo build && ./target/debug/crush generate_docs.crush
  > docs/builtins.html`) after the `pty` signature changes in Task 1, and again at the end
  if any other signature changed.
- `systemd` and `dbus` are Linux-only (`#[cfg(target_os = "linux")]` in
  `src/builtins/mod.rs`) and cannot be compiled or exercised on this (Darwin) dev machine.
  Task 5's Rust fix is written and self-reviewed but **cannot be built or run locally** —
  say so plainly when reporting that task's result, don't claim it passes. Its `.crush`
  test follows `tests/dbus.crush`'s own precedent: `if ($(host:os:name) != "linux") { exit
  }` as the first line, and no `.crush.output` file, so it's a clean no-op skip here and a
  real check on Linux CI.
- `remote:pexec`/`remote:exec` tests use the in-process test sshd fixture
  (`$(crush:env["SSHD_PORT"])`, wired up in `tests/system.rs`), never real network hosts.
- Don't modify any existing, already-passing `tests/pty_*.crush` file — every file this
  plan touches is new.

## Review Focus

- A fixed loop (`random:*_stream`, `fs:usage`) checking `control_receiver` too coarsely
  (e.g. once per directory instead of once per file) would still feel "stuck" for a
  visible moment after `terminate` on a large/slow case — each fix's test should use an
  input large enough that a coarse check would be visibly slower to stop than a fine one.
- `wait_quiet`'s poll loop must take `self.rx`'s lock and `self.parser`'s lock in the same
  order `drain_available`'s own doc comment requires (parser-then-rx, never held
  simultaneously in the other order) — a careless implementation that calls
  `self.contents()` (which locks both) from inside a loop that also separately locks `rx`
  is the easy way to reintroduce the AB-BA deadlock that comment warns about.
- Fixing `random:float_stream`/`random:integer_stream`'s loop must not change its output
  in the *uninterrupted* case — the existing `random.float_stream 100 | head 20`-style
  examples in its own doc comment must keep working identically.
- `systemd:journal`'s fix touches the `follow` branch only; the plain (non-follow) path's
  existing behavior (stop at end of journal, return `Ok(())`) must be provably unchanged —
  reason about this directly against the diff, since it can't be run locally.
- A test for a one-shot/bounded source command (Task 8) must not assert on timing it can't
  control (e.g. "this returns before the default timeout") — bounded commands only need to
  prove they still complete and their result is retrievable through `&`/`fg`, Ctrl-Z/Ctrl-C
  around them, not that job control interrupts them mid-flight (there is no "mid-flight").

---

## File Structure

```
src/builtins/pty/session.rs   — + PtySession::wait_quiet
src/builtins/pty/methods.rs   — + WaitQuiet, Run command structs/handlers
src/builtins/pty/create.rs    — + "wait_quiet", "run" added to the method-struct list

src/builtins/random.rs        — float_stream/integer_stream gain a registered controller
                                 + per-iteration control_receiver.try_recv() check
src/builtins/fs/usage.rs      — size() gains a &Receiver<StreamControlMessage> parameter,
                                 checked once per visited child
src/builtins/systemd.rs       — journal()'s follow branch polls with a timeout instead of
                                 blocking forever, checking a registered controller between
                                 polls (Linux-only; unverified locally)

tests/pty_test_helpers.crush  — sourced helper library (see spec's "Shared test helper
                                 library" section for the three helpers it provides)
tests/pty_helpers_smoke.crush(.output)              — proves the helpers themselves work
tests/pty_random_float_stream_job_control.crush(.output)
tests/pty_random_integer_stream_job_control.crush(.output)
tests/pty_fs_usage_job_control.crush(.output)
tests/pty_systemd_journal_follow_job_control.crush  — no .output file (see Global Constraints)
tests/pty_fs_watch_job_control.crush(.output)       — regression test, fs:watch was already correct
tests/pty_remote_pexec_job_control.crush(.output)   — regression test, pexec was already correct
tests/pty_<cmd>_job_control.crush(.output)          — one per Task 8 command (listed there,
                                                        12 bounded commands + pty_file_remove_job_control)

docs/builtins.html            — regenerated, never hand-edited
todo.md                       — struck items confirmed resolved by this plan (Task 9)
```

---

### Task 1: `pty` quiescence primitive (`wait_quiet` / `run`)

**Files:**
- Modify: `src/builtins/pty/session.rs`
- Modify: `src/builtins/pty/methods.rs`
- Modify: `src/builtins/pty/create.rs`
- Test: `tests/pty_helpers_smoke.crush`, `tests/pty_helpers_smoke.crush.output`

**Interfaces:**
- Produces: `PtySession::wait_quiet(&self, quiet: chrono::Duration, timeout: chrono::Duration) -> CrushResult<()>` in `session.rs`.
- Produces: two new pty session methods reachable from script as `$t:wait_quiet
  quiet=... timeout=...` and `$t:run $text quiet=... timeout=...`, both with
  `#[default(Duration::milliseconds(100))]` for `quiet` and `#[default(Duration::seconds(5))]`
  for `timeout`, matching `WaitFor`'s existing default-timeout convention.
- Consumes: the existing `POLL_INTERVAL` constant and `rx`/`parser` fields in
  `session.rs`; the existing `ChannelBasedController`/`Patterns` imports already used by
  sibling methods in `methods.rs`; `method_struct`'s existing loop over method names in
  `create.rs`.

- [ ] **Step 1: Implement `PtySession::wait_quiet` in `session.rs`**

Same poll loop shape as `wait_for` (`self.rx.lock().recv_timeout(POLL_INTERVAL)`, process
into `self.parser.lock()` on success, same parser-before-rx ordering — never call
`self.contents()`, which locks both at once, from inside this loop). Track the time of the
last chunk actually received; return `Ok(())` once `now - last_activity >= quiet`; return
`command_error` with the current `self.contents()` attached (same style as `wait_for`'s
timeout error) once `now >= deadline`.

- [ ] **Step 2: Add `WaitQuiet` and `Run` to `methods.rs`, register both in `create.rs`**

`WaitQuiet::parse` → call `session.wait_quiet(cfg.quiet, cfg.timeout)` → `Value::Empty`.
`Run::parse` → `session.write_raw(&(cfg.text + "\n"))?` → `session.wait_quiet(cfg.quiet,
cfg.timeout)` → `Value::Empty`. Add `"wait_quiet"` and `"run"` to the `for name in [...]`
list in `create.rs`'s `create()`, and to `methods()`'s declare calls in `methods.rs`.

- [ ] **Step 3: Write `tests/pty_helpers_smoke.crush`**

Spawn a nested crush (`pty:create $(crush:env["CRUSH_BIN"])`), `$t:wait_for "# "`, then
`$t:run "echo hello\n"` followed by `$t:wait_for "hello"` to prove `run` actually submits
and the session is usable afterward; separately, `$t:write $t:control:z` on a running
`sleep` job then a bare `$t:wait_quiet` (no text to match) to prove `wait_quiet` alone is
usable as a synchronization point before the next interaction. `$t:close`; `echo "ok"`.

- [ ] **Step 4: Build and run**

```bash
cargo build
./target/debug/crush tests/pty_helpers_smoke.crush
```

Expected: prints `ok`. Save that as `tests/pty_helpers_smoke.crush.output`.

```bash
cargo test
```

Expected: full suite still passes (confirms no regression from the signature changes).

- [ ] **Step 5: Regenerate docs and commit**

```bash
./target/debug/crush generate_docs.crush > docs/builtins.html
git add src/builtins/pty tests/pty_helpers_smoke.crush tests/pty_helpers_smoke.crush.output docs/builtins.html
git commit -m "pty: add wait_quiet/run quiescence primitives"
```

---

### Task 2: Shared pty test helper library

**Files:**
- Create: `tests/pty_test_helpers.crush`
- Test: exercised by every task from here on (no standalone test file of its own beyond Task 1's smoke test, which already covers the raw primitives these helpers wrap)

**Interfaces:**
- Produces: three sourced closures, exact names decided while writing this file (document
  the final names in this task's commit message) covering: (a) spawn a nested crush and
  return it once its first prompt is visible; (b) given a session and a name substring,
  the pid of the one matching child of `$t:pid` in `host:procs`, asserting exactly one
  match; (c) given a session and a job id, that job's current `status` string out of its
  own `jobs` table.
- Consumes: Task 1's `$t:run`/`$t:wait_quiet`, and the pre-existing `$t:pid`/`$t:wait_for`.

- [ ] **Step 1: Write the three helpers in `tests/pty_test_helpers.crush`, each with a comment on what it replaces and why (see spec's "Shared test helper library" section)**

- [ ] **Step 2: Prove each helper works by using all three from a throwaway scratch script run manually against the debug binary** (not committed — Task 3 onward is the real, committed usage)

```bash
cargo build
./target/debug/crush -c 'source "tests/pty_test_helpers.crush"; ...'
```

- [ ] **Step 3: Commit**

```bash
git add tests/pty_test_helpers.crush
git commit -m "tests: add shared pty job-control test helpers"
```

---

### Task 3: Fix + test `random:float_stream` / `random:integer_stream`

**Files:**
- Modify: `src/builtins/random.rs`
- Test: `tests/pty_random_float_stream_job_control.crush(.output)`,
  `tests/pty_random_integer_stream_job_control.crush(.output)`

**Interfaces:**
- Consumes: `ChannelBasedController`/`StreamControlMessage` from
  `crate::lang::job_control` (already imported this way in `fs/watch.rs`); Task 2's
  helpers.
- Produces: no new public interface — `float_stream`/`integer_stream` remain the same
  signature and output shape; only their loop body changes.

- [ ] **Step 1: Write the failing test for `random:integer_stream`**

In the nested session: `$t:run "random:integer_stream &\n"` (background it — nothing
pipes it into `head`, so today it would spin forever even after `terminate`), capture its
job id (id `0`, same precedent as other fresh-session pty tests), then `$t:run "terminate
0\n"`, then assert via the job-status helper that it reads `Terminated`, then assert (via
`crush:threads | where {job_id == 0} | count`, run inside the nested session and echoed
back, or equivalent) that **no thread is still running under job 0** — this is the part
that fails today, since `terminate` currently only stops *tracking* the thread, not the
thread itself.

- [ ] **Step 2: Run it, confirm it fails**

```bash
cargo build && ./target/debug/crush tests/pty_random_integer_stream_job_control.crush
```

Expected: the "no thread still running" assertion fails (thread count > 0).

- [ ] **Step 3: Fix `integer_stream` and `float_stream` in `random.rs`**

Register a `ChannelBasedController` via `context.command_handle().register(...)` before
the loop; inside the loop, before each `output.send(...)`, check
`control_receiver.try_recv()`: `Ok(Terminate)` → `return terminate()`; `Ok(Pause)` → inner
`loop { match control_receiver.recv() { ... } }` blocking until `Resume`/
`ResumeForeground` or `Terminate` (identical shape to `fs:watch`'s `Pause` arm);
`Ok(Resume)`/`Ok(ResumeForeground)` → no-op; `Err(TryRecvError::Disconnected)` →
`return terminate()`; `Err(TryRecvError::Empty)` → fall through to `output.send(...)` as
before.

- [ ] **Step 4: Run again, confirm it passes; write the float_stream test analogously; run it too**

```bash
cargo build
./target/debug/crush tests/pty_random_integer_stream_job_control.crush
./target/debug/crush tests/pty_random_float_stream_job_control.crush
```

Expected: both print `ok`. Save both `.crush.output` files.

- [ ] **Step 5: Confirm the uninterrupted case is unchanged**

```bash
echo 'random:float_stream 100 | head 5' | ./target/debug/crush
```

Expected: 5 rows, same shape as before this task (no behavior change when never
interrupted — this is Review Focus item 3).

- [ ] **Step 6: Full suite + commit**

```bash
cargo test
git add src/builtins/random.rs tests/pty_random_float_stream_job_control.crush tests/pty_random_float_stream_job_control.crush.output tests/pty_random_integer_stream_job_control.crush tests/pty_random_integer_stream_job_control.crush.output
git commit -m "random: make float_stream/integer_stream cooperate with job control"
```

---

### Task 4: Fix + test `fs:usage`

**Files:**
- Modify: `src/builtins/fs/usage.rs`
- Test: `tests/pty_fs_usage_job_control.crush(.output)`

**Interfaces:**
- Consumes: Task 2's helpers; `ChannelBasedController`/`StreamControlMessage`.
- Produces: `size()`'s signature grows one parameter:
  `control_receiver: &crossbeam::channel::Receiver<StreamControlMessage>`, threaded
  through its own recursive calls; `usage()` registers the controller once and passes the
  receiver down into every `size()` call.

- [ ] **Step 1: Write the failing test**

Build a synthetic directory tree under the outer test's own scratch dir large enough that
a full `fs:usage` scan visibly takes a few seconds even on a fast machine (e.g. a few
thousand small files across nested subdirectories — pick a count empirically while writing
this, large enough for Review Focus item 1 to actually bite a too-coarse check). In the
nested session: `$t:run "fs:usage $big_tree &\n"`, grab its job id, `$t:run "terminate
0\n"`, then assert the job's status reads `Terminated` *quickly* (well under the time a
full uninterrupted scan would take) using a bounded wait (e.g. retry the status check for
up to 1s, not a single fixed-delay check) — this is the part that fails today.

- [ ] **Step 2: Run it, confirm it fails (status check never goes `Terminated` in time, or the scan visibly keeps running)**

- [ ] **Step 3: Thread `control_receiver` through `size()`, register the controller in `usage()`**

Same `try_recv` shape as Task 3's fix, checked once per `child` in `size()`'s
`for child in lister.list(path)?` loop, before recursing into it.

- [ ] **Step 4: Run again, confirm it passes**

```bash
cargo build && ./target/debug/crush tests/pty_fs_usage_job_control.crush
```

- [ ] **Step 5: Confirm the uninterrupted case is unchanged**

```bash
./target/debug/crush -c 'fs:usage . | count'
```

Expected: same row count as before this task, on an uninterrupted run.

- [ ] **Step 6: Full suite + commit**

```bash
cargo test
git add src/builtins/fs/usage.rs tests/pty_fs_usage_job_control.crush tests/pty_fs_usage_job_control.crush.output
git commit -m "fs:usage: make the recursive scan cooperate with job control"
```

---

### Task 5: Best-effort fix for `systemd:journal --follow` (Linux-only, unverified locally)

**Files:**
- Modify: `src/builtins/systemd.rs`
- Test: `tests/pty_systemd_journal_follow_job_control.crush` (no `.crush.output` — see
  Global Constraints)

**Interfaces:**
- Consumes: `ChannelBasedController`/`StreamControlMessage`.
- Produces: no signature change — only `journal()`'s `follow` loop body changes.

- [ ] **Step 1: Change the `follow` branch to poll instead of blocking forever**

Register a controller before the loop. Replace `journal.await_next_entry(None)` (only
reached when `cfg.follow`) with `journal.await_next_entry(Some(short_poll_duration))` (pick
something in the low hundreds of ms — short enough that `terminate`/Ctrl-Z feel responsive,
long enough not to busy-spin). When that call returns `None` *and* `cfg.follow`, check
`control_receiver.try_recv()` (same `Terminate`/`Pause`/`Resume` handling as Tasks 3-4)
before looping back to wait again, instead of `break`ing (today's `None` + `!cfg.follow` →
`break` path is untouched).

- [ ] **Step 2: Write `tests/pty_systemd_journal_follow_job_control.crush`**

First line: `if ($(host:os:name) != "linux") { exit }` (matches `tests/dbus.crush`'s own
precedent exactly). Below that, the real check: background `systemd:journal
follow=$true`, `terminate` it, assert its job status reads `Terminated` and that no thread
is left running under its job id — same shape as Task 3's test, reusing Task 2's helpers.

- [ ] **Step 3: Confirm the skip path works here (this machine is Darwin)**

```bash
cargo build && ./target/debug/crush tests/pty_systemd_journal_follow_job_control.crush
```

Expected: exits cleanly with no output (the `host:os:name` guard fires). This is the only
part of this task verifiable on this machine — say so explicitly when reporting the task's
result; the Rust change itself is unverified until it's built and run on Linux.

- [ ] **Step 4: Reason through the diff by hand for the non-follow path**

Confirm (by reading, since it can't be run here) that `cfg.follow == false` still takes
the exact same `journal.next_entry()` → `None` → `break` path as before — the new polling
logic must be gated strictly behind `cfg.follow`.

- [ ] **Step 5: Commit, flagged clearly as unverified**

```bash
git add src/builtins/systemd.rs tests/pty_systemd_journal_follow_job_control.crush
git commit -m "systemd:journal: make --follow cooperate with job control (untested on macOS, Linux-only module)"
```

---

### Task 6: Regression test for `fs:watch`

**Files:**
- Test: `tests/pty_fs_watch_job_control.crush(.output)`

**Interfaces:** Consumes Task 2's helpers only; no source change (this command's job
control was already correct going in — confirmed in the spec's command inventory).

- [ ] **Step 1: Write the test** — background `fs:watch` on a scratch directory, confirm
  it reports a create event after a file is written into that directory, `terminate` it,
  assert `Terminated` status and no thread left running under its job id (same shape as
  Task 3).
- [ ] **Step 2: Run it, confirm it passes as-is** (this is a regression guard, not expected
  to find a bug).

```bash
cargo build && ./target/debug/crush tests/pty_fs_watch_job_control.crush
```

- [ ] **Step 3: Commit**

```bash
git add tests/pty_fs_watch_job_control.crush tests/pty_fs_watch_job_control.crush.output
git commit -m "tests: add pty job-control regression test for fs:watch"
```

---

### Task 7: Regression test for `remote:pexec`

**Files:**
- Test: `tests/pty_remote_pexec_job_control.crush(.output)`

**Interfaces:** Consumes Task 2's helpers and the existing sshd test fixture
(`$(crush:env["SSHD_PORT"])`, same credentials/host-file pattern as
`tests/ssh_pexec_partial_failure.crush`); no source change expected (already correct per
the spec's inventory — each worker registers its own controller via `run_remote`).

- [ ] **Step 1: Write the test** — background a `remote:pexec` call against the local test
  sshd with a command slow enough to still be running (e.g. a remote `sleep`), `terminate`
  the job, assert `Terminated` status and that the output stream ends (no further rows)
  promptly rather than after every host's sleep finishes.
- [ ] **Step 2: Run it**

```bash
cargo build && ./target/debug/crush tests/pty_remote_pexec_job_control.crush
```

If it fails, that's a real finding outside this plan's original inventory — fix it the
same way as Task 3/4 (register + `try_recv`/`select!` check in whichever loop doesn't
already have one) before moving on, and note in the commit message that the inventory's
"already correct" classification was wrong.

- [ ] **Step 3: Commit**

```bash
git add tests/pty_remote_pexec_job_control.crush tests/pty_remote_pexec_job_control.crush.output
git commit -m "tests: add pty job-control regression test for remote:pexec"
```

---

### Task 8: Smoke tests for the remaining bounded source commands

**Files:**
- Test, one file (+ `.output`) each: `sockets:tcp`, `sockets:udp`, `host:battery`,
  `host:procs`, `host:threads`, `groups:list`, `users:list`, `remote:identity`,
  `remote:host:list`, `fs:stat`, `fs:mounts`, `crush:history`, a `file` value's
  `:remove recursive=$true` method (`types.file.remove` — invoked on a value, not a bare
  command; see spec) — named `tests/pty_<command-with-underscores>_job_control.crush`
  (the last one as `tests/pty_file_remove_job_control.crush`).

**Interfaces:** Consumes Task 2's helpers only. No source change expected for the first
twelve (bounded, nothing to interrupt mid-flight — see Review Focus's last item). For
`:remove`: if the test shows a terminate during a large recursive delete doesn't stop it
early, apply the same `try_recv`-per-visited-entry fix as Task 4 before committing;
otherwise just commit the test.

- [ ] **Step 1: Write and run one test per command**

Each test: background the command (`&`) inside a nested pty session (using whatever
arguments make it meaningful — e.g. `host:battery` needs no args; `remote:host:list`/
`remote:identity` need the same local fixtures Task 7 uses if they require one; `:remove`
needs a throwaway file/tree to delete), confirm its result is retrievable via `fg` once
resumed, and confirm sending Ctrl-Z then `bg` then `fg` around it doesn't hang or corrupt
the session. For `:remove` specifically, also check whether `terminate` on a large
recursive delete stops it early (same technique as Task 4's size-check, adapted to "how
many of N pre-created files are still present").

```bash
cargo build
for f in tests/pty_sockets_tcp_job_control.crush tests/pty_sockets_udp_job_control.crush ...; do
  ./target/debug/crush "$f"
done
```

- [ ] **Step 2: If `types.file.remove` needs the fix, apply it (mirror Task 4) and re-run that one test**

- [ ] **Step 3: Full suite + commit**

```bash
cargo test
git add tests/pty_sockets_tcp_job_control.crush tests/pty_sockets_tcp_job_control.crush.output ... # every file from Step 1
git commit -m "tests: add pty job-control smoke tests for the remaining bounded stream-source commands"
```

---

### Task 9: Final sweep

**Files:**
- Modify: `docs/builtins.html` (regenerated)
- Modify: `todo.md` (strike resolved items, if confirmed)

**Interfaces:** None — verification and bookkeeping only.

- [ ] **Step 1: Full rebuild and test**

```bash
cargo build && cargo test
```

Expected: everything passes, including every new file from Tasks 1-8.

- [ ] **Step 2: Regenerate docs**

```bash
./target/debug/crush generate_docs.crush > docs/builtins.html
```

- [ ] **Step 3: Check `todo.md`'s job-control-adjacent lines against what this plan actually did**

Lines 13 (`Add signal handlers to fix ^C and ^Z during regular execution`), 28
(`Implement job control for timeit`), 30 (`Job control for cmd command`), 31/32
(`Document what causes the next job to execute` / `Document job control`) predate this
plan and may already be resolved by earlier work (commits `ea7762f2`..`97e790e5`) or by
this plan's own Tasks 3-8 — verify each individually (grep its own area's tests/code, the
same way this plan's research did) before striking it; don't strike on assumption.

- [ ] **Step 4: Commit**

```bash
git add docs/builtins.html todo.md
git commit -m "docs: regenerate builtins reference, update todo.md for resolved job-control items"
```
