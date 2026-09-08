# Test coverage gaps

Found via a four-way code audit (language core/execution, data model/serialization,
types/stream/control builtins, system/io/network builtins + the `signature` macro).
Each item was checked against `tests/*.crush` and existing `#[cfg(test)]` blocks before
being listed here — these are confirmed gaps, not guesses. Check items off as they get a
test (and, where noted, a fix).

Two items already overlap with existing entries in `todo.md`: "Add system tests for binary
stream handling" and "Write tests that use `schedule` and job control".

## Silent-corruption bugs (no crash, just wrong data)

- [ ] `pup` serialization drops a struct's parent chain — `src/lang/serialization/struct_serializer.rs:70-79`.
      `serialize()` hardcodes `parent: None`, ignoring the struct's real parent. Any
      `class()`-based object (inheritance, custom methods, `__setattr__`) sent through
      `pup:to`/`sudo`/`remote:exec` silently degrades to a bare data struct on the other
      side — it looks fine until you call an inherited method. `tests/serialization.crush`
      only round-trips a plain Table/Row, never a class instance.
- [ ] `pup` serialization truncates `Duration` to whole seconds — `src/lang/serialization/value_serializer.rs:160-170`.
      `nanos` is zeroed unconditionally even though the wire format and the deserializer
      both support it. Any sub-second duration silently loses precision crossing a
      `--pup` boundary.
- [ ] `stream/join.rs` column-collision renaming is unexercised (lines ~56-75) — when both
      sides share a non-key column name, output columns get renamed `_2`, `_3`, etc.
      `tests/join.crush`'s only shared column is the join key itself, so this logic has
      never actually run. Also unverified: right-side rows with no left match are
      silently dropped, and duplicate left keys fanning out — nothing confirms `join`
      behaves like a real inner join with correct multiplicity.
- [ ] `stream/aggregation.rs` mixed Integer+Float columns fall through `sum_any`/`avg_any`'s
      type-tracking match to an unverified catch-all — could be silent data loss rather
      than a sensible error.
- [ ] Float `NaN`/`±0.0`/`±inf` in comparisons/sort/dedup have no test coverage in
      `src/lang/value/mod.rs`'s `PartialEq`/`PartialOrd`, which back `sort`, `==`, and
      dict/table keys. Silently wrong order or dedup, not a crash.

## Reachable panics (should be `CrushResult` errors, aren't)

- [ ] `InterruptibleTableInputStream::read`, `src/lang/pipe.rs:284` — a `Resume` control
      message arriving while not paused hits a bare `panic!()`. No test drives job-control
      (pause/resume/terminate) signals through a stream at all.
- [ ] `control/schedule.rs` — same `Resume`-outside-pause `panic!()` pattern in its
      `sleep()`, plus a fixed-rate catch-up mode (`next_duration = last_time - Local::now()`,
      skips sleep if overrun) that's classic drift logic with zero coverage of any kind —
      no `.crush` file even mentions `schedule`.
- [ ] `stream/aggregation.rs::median_*` (lines 207-211) indexes `res[...]` directly with no
      empty-check — `median` (or `avg`/`min`/`max`, also untested) on an empty stream
      underflows/panics instead of erroring.
- [ ] `stream/sort.rs` — an incomparable pair (e.g. NaN) hits
      `panic!("Unexpected sort failure")` rather than a graceful error; nothing sorts a
      column that could produce `None` from `partial_cmp`.

## Likely deadlock

- [ ] Pipeline whose last command errors before producing output —
      `src/lang/job.rs:62-70` + `src/lang/command_invocation.rs:256-262`. The non-blocking
      eval path swallows command errors via `printer().handle_error()` and returns
      `Ok(None)`, but `Job::eval` then unconditionally does
      `context.output.send(last_input.recv()?)`. If the last stage never sent anything,
      that `recv()` blocks forever with no sender left. This is the exit path of every
      pipeline, and "last command fails" is never tested.

## Security-relevant, untested

- [ ] `remote.rs` SSH host-key verification (`exec`/`pexec`, ships a serialized closure
      over SSH and deserializes the result) — `known_hosts.check_port` handling for
      not-found/mismatch, the TOFU auto-add path (`allow_not_found`), and
      `ignore_host_file` are all untested. A bug here is MITM-adjacent, not just a
      correctness nit.
- [ ] `dbus.rs` (843 lines) and `systemd.rs` — Linux-only, zero coverage on any platform
      (can't even be exercised in CI on macOS/most dev machines).
- [ ] `dns.rs` — real UDP queries with response parsing against attacker-influenceable
      input (reverse-DNS lookups); no malformed/truncated-response handling is verified.

## Framework code everything else depends on

- [ ] `signature` crate's argument-binding algorithm — `signature/src/lib.rs:375-507`.
      Generates the parser for essentially every builtin (~150+ commands) and has zero
      tests of its own. The binding order has a genuine subtlety: once an `#[unnamed()]`
      collector field is seen, later fields only still consume positional args if they
      *also* have a `#[default(...)]`. A regression here silently mis-binds arguments
      across the whole command surface.

## Test infrastructure gaps

- [ ] `run_system_test` in `tests/system.rs` compares expected vs. actual output via
      `expected_lines.iter().zip(actual_lines.iter())`, which silently stops comparing at
      the shorter of the two — if a regression makes a script produce *fewer* lines than
      expected (e.g. a top-level statement now errors and `source()` in
      `src/lang/execute.rs` aborts the rest of the script), the missing/extra lines are
      never checked and the golden test can pass even though the output is wrong. Found
      while writing a repro for the struct-parent-in-pup bug above: a naive `.crush`
      golden test for that bug would have passed today despite the bug being present,
      because the buggy run produces empty output rather than a differing line. Fix
      should assert `actual_lines.len() == expected_lines.len()` (or equivalent) in
      addition to the per-line comparison. Deliberately treated as a separate task from
      any specific bug fix.

## Untested control-flow / stream ops (lower severity, still real gaps)

- [ ] `control/while.rs` — completely untested (no `while` in any `.crush` file), including
      its documented "no body -> condition is the body" alternate mode.
- [ ] `stream/group.rs` — only single-key, non-empty grouping is exercised; multi-column
      grouping, empty-stream grouping, and aggregator-command failure inside the spawned
      worker thread are not.
- [ ] `stream/uniq.rs` whole-row dedup (`field: None`, hashing an entire `Row` including
      floats/structs) is untested — only column-based uniq is covered.
- [ ] `types/re.rs` / `one_of.rs` — no dedicated test file at all despite regex
      capture/replace and multi-pattern matching being nontrivial.
- [ ] `(expr)` -> synthetic `val` desugaring and `[...]` list-literal desugaring
      (`src/lang/ast/node.rs`) — this exact mechanism is what caused the real completion
      bug fixed earlier this session. `list_literal` uses a structurally similar
      synthetic-command trick and has no test for nested `(expr)` inside `[...]`, empty
      `[]`, or interaction between the two.
- [ ] `closure.rs` (1323 lines, largest file in the crate) has zero direct unit tests;
      only indirectly covered via `tests/closure_signatures.crush`.
