# Test coverage gaps

Found via a four-way code audit (language core/execution, data model/serialization,
types/stream/control builtins, system/io/network builtins + the `signature` macro).
Each item was checked against `tests/*.crush` and existing `#[cfg(test)]` blocks before
being listed here — these are confirmed gaps, not guesses. Check items off as they get a
test (and, where noted, a fix).

Two items already overlap with existing entries in `todo.md`: "Add system tests for binary
stream handling" and "Write tests that use `schedule` and job control".

## Silent-corruption bugs (no crash, just wrong data)

- [x] `pup` serialization drops a struct's parent chain — `src/lang/serialization/struct_serializer.rs:70-79`.
      `serialize()` hardcodes `parent: None`, ignoring the struct's real parent. Any
      `class()`-based object (inheritance, custom methods, `__setattr__`) sent through
      `pup:to`/`sudo`/`remote:exec` silently degrades to a bare data struct on the other
      side — it looks fine until you call an inherited method. `tests/serialization.crush`
      only round-trips a plain Table/Row, never a class instance.
      Fixed: added `Struct::parent()` (`src/lang/data/struct.rs`) and made
      `serialize()` encode the real parent via `ParentValue(...)`. Covered by
      `struct_parent_survives_pup_round_trip` in `struct_serializer.rs`, plus a manual
      end-to-end check through the interpreter with a pure custom class hierarchy. Note:
      exposed a separate bug in the process — see "`Command::deserialize` uses the wrong
      element index" below, which still blocks the common case of `class()`'s *default*
      parent (`scope.root_object()`, which holds native builtin commands).
- [ ] `pup` serialization truncates `Duration` to whole seconds — `src/lang/serialization/value_serializer.rs:160-170`.
      `nanos` is zeroed unconditionally even though the wire format and the deserializer
      both support it. Any sub-second duration silently loses precision crossing a
      `--pup` boundary.
- [ ] `stream/join.rs` — right-side rows with no left match are silently dropped, and
      duplicate left keys fanning out, are both still unverified — nothing confirms
      `join` behaves like a real inner join with correct multiplicity.
      Column-collision renaming (originally flagged here as unexercised, since
      `tests/join.crush`'s only shared column is the join key itself) is no longer a
      gap: `get_output_type` now calls the shared `ColumnVec::deduplicate_names()`
      (`src/lang/data/table.rs`), which is directly exercised by `tests/zip.crush`,
      `tests/group.crush` and `tests/select.crush` — see the `test_zip` entry below for
      the full story of how that surfaced.
- [ ] `stream/aggregation.rs` mixed Integer+Float columns fall through `sum_any`/`avg_any`'s
      type-tracking match to an unverified catch-all — could be silent data loss rather
      than a sensible error.
- [ ] Float `NaN`/`±0.0`/`±inf` in comparisons/sort/dedup have no test coverage in
      `src/lang/value/mod.rs`'s `PartialEq`/`PartialOrd`, which back `sort`, `==`, and
      dict/table keys. Silently wrong order or dedup, not a crash.

## Reachable bugs found while fixing other items on this list

- [ ] `Command::deserialize` uses the wrong element index —
      `src/lang/command/mod.rs:228-229`. `serialize()` for a native command emits
      `element::Element::Command(strings_idx)`, where `strings_idx` points at a separate
      `Element::Strings` holding the command's full path (e.g. `["global", "io",
      "echo"]`). `deserialize()` matches `element::Element::Command(_)` and **discards**
      that index, then calls `Vec::deserialize(id, ...)` reusing the *outer* command
      element's own `id` — which points at the `Command` element, not the `Strings`
      element — so it always fails with `Expected string list`. This path was
      apparently never reachable by any existing test, because nothing ever
      pup-serialized a literal `Value::Command` before. Surfaced while fixing the
      struct-parent bug above: `class()`'s default parent is `scope.root_object()`,
      whose fields are native `Command`s, so `pup:to`/`pup:from` (and therefore
      `sudo`/`remote:exec`) on *any* ordinary `class()`-based struct with default
      inheritance still fails today, just with a loud error instead of silent data
      loss. Minimal fix looks like capturing `strings_idx` from the match arm instead
      of `id`.

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
- [x] `stream/sort.rs` — an incomparable pair (e.g. NaN) hit
      `panic!("Unexpected sort failure")` rather than a graceful error; nothing sorted a
      column that could produce `None` from `partial_cmp`.
      Fixed: added `compare_for_sort()`, giving NaN a defined position (always sorts as
      the greatest value — last ascending, first descending) instead of panicking.
      Covered by `tests/sort_nan.crush`.

## Pipeline whose last command errors before producing output

- [ ] `src/lang/job.rs:62-70` + `src/lang/command_invocation.rs:256-262`. The
      non-blocking eval path swallows command errors via `printer().handle_error()` and
      returns `Ok(None)`, but `Job::eval` then unconditionally does
      `context.output.send(last_input.recv()?)`. **Correction:** originally flagged here
      as a likely deadlock — verified by hand (with a timeout guard) and it is not one.
      `crossbeam::channel::Receiver::recv()` returns `Err` as soon as every `Sender` is
      dropped rather than blocking forever, so this resolves almost instantly. The real,
      confirmed bug: that `RecvError` (`"receiving on an empty and disconnected
      channel"`, from `crossbeam::channel::RecvError`'s own `Display`, wrapped by
      `CrushErrorType::RecvError` in `src/lang/errors.rs` — note `is_disconnected()`
      already exists there as a way to recognize this specific error class) gets
      propagated as *the* job error via the trailing `?`, silently replacing/burying the
      real error that was already printed a moment earlier by `handle_error()`. Every
      pipeline whose last command fails leaks this confusing second, unrelated message —
      verified identically on both the non-blocking path (`convert` erroring
      synchronously) and the blocking/threaded path (a failing `select` call).
      Reproduced in `tests/error_handling/last_command_error.crush`, asserted by
      `test_last_command_error_does_not_leak_a_stray_channel_error` in
      `tests/system.rs` (currently red). Deliberately out of scope for now: whether a
      failing last command should also make the process exit non-zero (currently exits
      0) — punted as a separate, unresolved design question, not asserted by the test.

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

- [x] `signature` crate's argument-binding algorithm — `signature/src/lib.rs:375-507`.
      Generates the parser for essentially every builtin (~150+ commands) and had zero
      tests of its own. Added `src/lang/signature_binding_tests.rs` (18 tests): baseline
      named/unnamed binding order, duplicate named args, stray unnamed args, type
      mismatches, `#[unnamed()]` `Vec` collectors, and specifically the flagged
      subtlety — a field declared *after* an `#[unnamed()]` collector. Traced the
      generated code precisely: the collector's `while !_unnamed.is_empty()` drain runs
      before any later field's own binding code (mutate blocks are emitted in struct
      declaration order and execute sequentially), so such a field can only ever be
      filled by name — a `#[default(...)]` on it doesn't let it "reach past" the
      collector and steal a positional value, it only changes what happens when nothing
      names it (falls back to the default) vs. when it's required (errors clearly). All
      18 tests pass against the current implementation — no bug found this pass, but
      this was previously completely unverified and is exactly the kind of thing a
      refactor could silently break. `#[named()]` collectors were also checked and don't
      have the same effect on later fields (they don't set the same internal flag), which
      is what `control::for`'s `For` struct relies on in production (`#[named()]
      iterator` followed by a plain positional `body`) — also now covered.
      No production code was touched, per instruction.

## Test infrastructure gaps

- [x] `test_zip` (`tests/zip.crush`) had apparently been silently broken for a while,
      masked by a gap in `run_system_test` itself (its expected-vs-actual comparison
      used `expected_lines.iter().zip(actual_lines.iter())`, which silently stopped at
      the shorter of the two — since fixed with an added length check, and covered by
      two tests in `tests/system.rs` using fixtures under `tests/harness/`): `zip $(lines:from
      ./example_data/age.csv|...) $(lines:from ./example_data/home.csv|...)` errored with
      `global:stream:zip: Duplicate column name, column 0 and column 1 are both named
      'line'`, from the duplicate-column-name check in `src/lang/pipe.rs:334-343` —
      `streams()`'s validation was added (commit `3489e7a`) well after `zip.crush` was
      last written, a genuine regression. Investigating turned out the same
      no-collision-handling pattern also existed in `select` and `group` (see the
      `stream/join.rs` entry above, now folded into the fix). Fixed: all three now auto-rename
      colliding columns via the new `ColumnVec::deduplicate_names()`, matching `join`'s
      existing behavior. Covered by `tests/zip.crush` (updated), `tests/group.crush`
      (new collision case added) and `tests/select.crush` (new).

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
