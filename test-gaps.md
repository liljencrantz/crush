# Test coverage gaps

Found via a four-way code audit (language core/execution, data model/serialization,
types/stream/control builtins, system/io/network builtins + the `signature` macro).
Each item was checked against `tests/*.crush` and existing `#[cfg(test)]` blocks before
being listed here — these are confirmed gaps, not guesses. Check items off as they get a
test (and, where noted, a fix).

One item already overlapped with an existing entry in `todo.md`, "Write tests that use
`schedule` and job control" — done via `tests/schedule.crush` and `tests/bg_fg.crush`,
removed from `todo.md`. A second, "Add system tests for binary stream handling", is still
open in `todo.md`.

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
      element index" below (now fixed), which had blocked the common case of `class()`'s
      *default* parent (`scope.root_object()`, which holds native builtin commands).
- [x] `pup` serialization truncates `Duration` to whole seconds — `src/lang/serialization/value_serializer.rs:160-170`.
      `nanos` was zeroed unconditionally even though the wire format and the deserializer
      both support it. Any sub-second duration silently lost precision crossing a
      `--pup` boundary (e.g. via `sudo`, `remote:exec`, or `users:me:do`).
      Fixed: serialize side now uses `chrono::TimeDelta::subsec_nanos()`, which pairs
      with `num_seconds()` (truncate-toward-zero) to exactly match the reconstruction
      formula already used on the deserialize side. Covered by
      `tests/duration_via_pup.crush` (500ms through `users:me:do`, confirmed red before
      the fix, green after).
- [x] `stream/join.rs` — right-side rows with no left match were silently dropped, and
      duplicate left keys fanning out, were both unverified — nothing confirmed `join`
      behaved like a real inner join with correct multiplicity. Covered by the second
      case added to `tests/join.crush`: a key duplicated on the left, duplicated on the
      right, present on only the left, present on only the right, and a plain 1:1 key as
      a baseline. No bug found — `do_join` already buckets the left stream by key into
      `Vec<Row>` (preserving left duplicates) and streams the right side row by row,
      fanning out over all matching left rows per right row, exactly matching real
      inner-join semantics. This was purely a coverage gap.
      Column-collision renaming (originally flagged here as unexercised, since
      `tests/join.crush`'s only shared column is the join key itself) is no longer a
      gap: `get_output_type` now calls the shared `ColumnVec::deduplicate_names()`
      (`src/lang/data/table.rs`), which is directly exercised by `tests/zip.crush`,
      `tests/group.crush` and `tests/select.crush` — see the `test_zip` entry below for
      the full story of how that surfaced.
- [x] `stream/aggregation.rs` mixed Integer+Float columns fall through `sum_any`/`avg_any`'s
      type-tracking match to a catch-all — confirmed it's a correct error, not silent
      data loss. Covered by `tests/aggregation_mixed_types.crush`: mixed Integer+Float,
      Integer+Duration, and Float+Duration all correctly error for `sum`/`avg`, a
      non-numeric value mixed in correctly errors too, and `min`/`max`/`median`/`prod`
      all reject `$any`-typed columns unconditionally (no `$any` dispatch arm), so mixed
      types trivially error there as well. One real bug found and fixed separately:
      `avg_any`'s mismatch error message was copy-pasted from `sum_any` verbatim
      ("Received multiple types in sum" even when the mismatch was in `avg`) — cosmetic
      only, didn't affect whether it errored, fixed to say "average".
- [x] Float `NaN`/`±0.0`/`±inf` in comparisons/sort/dedup had no test coverage in
      `src/lang/value/mod.rs`'s `PartialEq`/`PartialOrd`/`Hash`, which back `sort`, `==`,
      and dict/table keys. Covered by `tests/comparisons.crush` (`==`/`!=` for all three,
      plus ordering operators for `±0.0`/`±inf`), `tests/sort_nan.crush` (extended with
      `±0.0`/`±inf`), and the new `tests/float_edge_case_dedup.crush`.
      A real bug turned up: `Value`'s `Hash` impl decomposed a float's raw bits
      including the sign, so `+0.0` and `-0.0` — `==` per `PartialEq`/IEEE 754 — hashed
      differently, violating the Hash/Eq contract (`a == b` must imply
      `hash(a) == hash(b)`). This broke `uniq` (both the whole-value and column-based
      dedup paths) and `Dict`, which both treated `+0.0`/`-0.0` as distinct when they
      should collapse to one. Fixed by canonicalizing `-0.0` to `0.0` before hashing.
      Also confirmed, as expected and not bugs: ordering comparisons (`>`/`<`/`>=`/`<=`)
      on `NaN` error rather than returning a bool, since `partial_cmp` returns `None`
      for it and `comp.rs`'s `cmp!` macro treats that as an error; `sort` gives `NaN` a
      defined last position (already fixed earlier this session) and leaves `±0.0` in
      their stable-sort relative order; `uniq` never dedups identical `NaN` values
      (matches `NaN != NaN`) and correctly dedups `+infinity` with itself while keeping
      `-infinity` distinct.

## Reachable bugs found while fixing other items on this list

- [x] `Command::deserialize` uses the wrong element index —
      `src/lang/command/mod.rs:228-229`. `serialize()` for a native command emits
      `element::Element::Command(strings_idx)`, where `strings_idx` points at a separate
      `Element::Strings` holding the command's full path (e.g. `["global", "io",
      "echo"]`). `deserialize()` matched `element::Element::Command(_)` and **discarded**
      that index, then called `Vec::deserialize(id, ...)` reusing the *outer* command
      element's own `id` — which points at the `Command` element, not the `Strings`
      element — so it always failed with `Expected string list`. This path was
      apparently never reachable by any existing test, because nothing ever
      pup-serialized a literal `Value::Command` before. Surfaced while fixing the
      struct-parent bug above: `class()`'s default parent is `scope.root_object()`,
      whose fields are native `Command`s, so `pup:to`/`pup:from` (and therefore
      `sudo`/`remote:exec`) on *any* ordinary `class()`-based struct with default
      inheritance had failed too, just with a loud error instead of silent data loss.
      Fixed: capture `strings_idx` from the match arm and deserialize that element
      instead of the outer one. Covered by `tests/command_value_via_pup.crush`, which
      round-trips a bare `global:io:echo` reference through `pup:to`/`pup:from`
      in-process (no subprocess needed to hit the same code path) and echoes a marker
      afterward — since an uncaught error aborts the rest of the script (`source()`
      propagates a job's `Err` via `?`), the marker only appears once the round trip
      actually succeeds, giving a plain stdout diff. Confirmed red before the fix,
      green after.

- [x] `control/schedule.rs`'s piped-input branch did `output.send(input.read()?)` with no
      bound on the input; once the input stream was exhausted, `input.read()` returned
      Err (a crossbeam `RecvError` — the row channel carries plain `Row`, not
      `Result<Row, _>`, so a disconnect can only ever mean "no more rows"), and the `?`
      propagated it as a spurious "receiving on an empty and disconnected channel" error
      instead of stopping cleanly, unlike every other `TableStreamReader` consumer in the
      codebase (`while let Ok(row) = ... .read() { }`, which already treats any error as
      "stop"). Fixed by generalizing: added `TableStreamReader::next_row()`, a default
      method mapping `CrushError::is_disconnected()` to `Ok(None)` and propagating
      everything else as a real `Err`, and migrated every read-loop call site (~20 files)
      to `while let Some(row) = ... .next_row()? { }`. Covered by
      `tests/error_handling/schedule_exhausted_input.crush` (a custom Rust assertion on
      stderr, not a plain stdout diff — the error happens on the pipeline's own
      background thread and is caught/printed by `command_invocation.rs`'s
      `eval_command` per-stage, not propagated as a hard job failure, so there's no
      synchronous point for a stdout marker, unlike the `Command::deserialize` case
      above). Confirmed red before the fix, green after.
      **Follow-up, not done as part of this fix:** the generalization is deliberately
      scoped to fixing ordinary stream exhaustion, which was the one thing actually
      broken. It does *not* add test coverage proving that the other two things
      `next_row()` now propagates as real errors — an explicit `Terminate` interrupt
      arriving mid-read, and a genuine data/validation error from
      `TableInputStream::recv()`'s schema check — actually behave correctly at each of
      the ~20 migrated call sites. Before this fix, both were silently swallowed as if
      the stream had just ended cleanly everywhere (e.g. an interrupted `sort | head`
      would quietly return a normal, just-truncated result instead of aborting); after
      this fix, they propagate as a real `Err` from `next_row()?`, which should be
      correct, but that's reasoned from the code, not verified per-site. A validation
      error should never fire in practice (both ends of a pipe agree on schema when it's
      created), so this is a defense-in-depth concern more than a live bug.
      **Related, unexplained:** investigating this surfaced that piping the schedule
      output into `count` (`seq 0 2 | schedule $(duration:of milliseconds=1) | count`)
      produced *no* output at all on the buggy code, not even a partial/wrong count —
      `count` computes `res` internally without ever seeing an error (it already used
      `while let Ok(_) = input.read() { }`), so its own `context.output.send(...)` should
      have run and produced `3`. Something at the job/pipeline level appears to suppress
      a stage's otherwise-successful output when an *earlier*, non-last stage in the same
      pipeline errored, which would be a distinct mechanism from both this bug and the
      already-fixed "last command errors" stray-channel-error bug. Not investigated
      further — noted here in case it recurs.

## Reachable panics (should be `CrushResult` errors, aren't)

- [x] `stream/uniq.rs` — `Value::hash()` has a guard
      (`if !self.value_type().is_hashable() { panic!(...) }`) meant to be prevented by
      callers checking `is_hashable()` first (`sort.rs` does this via `is_comparable()`
      before comparing a column), but `uniq.rs` had no equivalent check before calling
      `HashSet::contains`/`insert` on a `Row` (whole-row path) or a `Value` (column
      path), so deduplicating a stream with a `Struct`- or `List`-typed column crashed
      the worker thread directly with "Can't hash mutable cell types!". Found while
      adding whole-row dedup coverage for `stream/uniq.rs` above.
      Fixed: check each value's actual runtime type before hashing, returning a graceful
      `command_error` instead. Has to be a per-value check, not a single check against
      the column's declared type — a `select`-computed column is always statically
      declared as `$any` (a closure's output type is never known ahead of time), so the
      unhashable type only exists at runtime, the same reason `sum_any`/`avg_any` check
      per-row. Covered by `tests/error_handling/uniq_unhashable_type.crush` — a custom
      Rust assertion on stderr, since a panicking thread and a graceful error both leave
      stdout empty, and whether the panic message reaches stderr before the whole
      process exits turned out to be a race in a longer script (confirmed by hand),
      hence keeping it as its own short, isolated script rather than folding it into a
      larger combined test.
- [x] `InterruptibleTableInputStream::read`, `src/lang/pipe.rs:284` — a `Resume` control
      message arriving while not paused hit a bare `panic!()`. No test drove job-control
      (pause/resume/terminate) signals through a stream at all.
      Fixed: the `select!` loop moved inside an outer `loop {}`, and an unexpected
      `Resume` now just falls through to the next iteration (silently ignored) instead
      of panicking. Verified by hand: backgrounded `files --recurse / | echo`, then
      `crush:resume jid=<id>` on it *without* ever pausing first — no panic, script
      continued normally. Not yet wired into an automated test (would need to drive
      job-control messages from a `.crush` script, which isn't easily expressible
      today) — the manual repro above is the only verification.
- [x] `control/schedule.rs` had the same `Resume`-outside-pause `panic!()` pattern in
      its `sleep()`.
      Fixed the same way (wrap in an outer `loop {}`, `Resume` falls through instead of
      panicking). Verified by hand: backgrounded `schedule $(duration:of seconds=10)`,
      then `crush:resume jid=<id>` on it while it was still in its initial sleep — no
      panic.
      **Correction on the rest of this item:** the fixed-rate catch-up logic
      (`next_duration = last_time - Local::now()`, skipping `sleep` if overrun) is *not*
      a bug — `schedule_at_fixed_rate` is a real, documented, opt-in mode (default
      `false`; the default mode always sleeps the full interval regardless of how long
      the previous heartbeat took). `last_time` accumulates from a fixed baseline
      (`t0, t0+interval, t0+2*interval, ...`), so skipping the sleep when behind and
      firing immediately is exactly the documented "catch up by sending more heartbeats
      afterwards" behavior, matching e.g. Java's `scheduleAtFixedRate`. Mischaracterized
      this as "classic drift logic" without first checking there was a documented mode
      governing it. `schedule` now has coverage for its ordinary (non-fixed-rate) usage
      forms via `tests/schedule.crush`; `schedule_at_fixed_rate` itself is still
      untested.
- [x] `stream/aggregation.rs` on an empty stream — audited every aggregator by hand via
      `tests/aggregation_empty.crush`. **Correction:** `median_*` already had an explicit
      `res.is_empty()` check returning a clean error (the original note that it indexes
      `res[...]` unchecked was wrong, or true of an older version of the code). The real,
      confirmed bugs: `avg_int`/`avg_float`/`avg_duration` (the `avg_function!` macro)
      divided by the row count unconditionally, panicking with "attempt to divide by
      zero" on an empty stream; `min`/`max` (`aggr_function!`) didn't panic but leaked
      the raw `s.read()?` `RecvError` ("receiving on an empty and disconnected channel")
      instead of a clear message. `sum`/`prod`/`concat` were already fine (correctly
      return `0`/`1`/`""`). Fixed in two commits: `40e812d` (avg: explicit `count == 0`
      check before dividing) and `ef7c37b` (min/max: explicit `s.read()` check up front
      with a proper "Can't calculate {min,max} of empty set" message).
      `tests/aggregation_empty.crush` exercises all of them by hand; not wired into an
      automated assertion (each erroring line aborts the rest of the script under the
      job.rs bug fixed just above, which made testing several aggregators in one file
      that way impractical).
- [x] `stream/sort.rs` — an incomparable pair (e.g. NaN) hit
      `panic!("Unexpected sort failure")` rather than a graceful error; nothing sorted a
      column that could produce `None` from `partial_cmp`.
      Fixed: added `compare_for_sort()`, giving NaN a defined position (always sorts as
      the greatest value — last ascending, first descending) instead of panicking.
      Covered by `tests/sort_nan.crush`.

## Pipeline whose last command errors before producing output

- [x] `src/lang/job.rs:62-70` + `src/lang/command_invocation.rs:256-262`. The
      non-blocking eval path swallows command errors via `printer().handle_error()` and
      returns `Ok(None)`, but `Job::eval` then unconditionally did
      `context.output.send(last_input.recv()?)`. **Correction:** originally flagged here
      as a likely deadlock — verified by hand (with a timeout guard) and it is not one.
      `crossbeam::channel::Receiver::recv()` returns `Err` as soon as every `Sender` is
      dropped rather than blocking forever, so this resolves almost instantly. The real,
      confirmed bug: that `RecvError` (`"receiving on an empty and disconnected
      channel"`, from `crossbeam::channel::RecvError`'s own `Display`, wrapped by
      `CrushErrorType::RecvError` in `src/lang/errors.rs` — note `is_disconnected()`
      already exists there as a way to recognize this specific error class) got
      propagated as *the* job error via the trailing `?`, silently replacing/burying the
      real error that was already printed a moment earlier by `handle_error()`. Every
      pipeline whose last command failed leaked this confusing second, unrelated
      message — verified identically on both the non-blocking path (`convert` erroring
      synchronously) and the blocking/threaded path (a failing `select` call).
      Fixed (commit `ef7c37b`): only forward `last_input.recv()`'s value downstream if it
      actually arrives; a disconnect is now treated as "the last command produced
      nothing" rather than a fresh error, and whatever `last_call_def.eval()` actually
      returned decides the job's own result. Reproduced in
      `tests/error_handling/last_command_error.crush`, asserted by
      `test_last_command_error_does_not_leak_a_stray_channel_error` in
      `tests/system.rs` — now passing. Deliberately out of scope: whether a failing last
      command should also make the process exit non-zero (currently exits 0) — punted as
      a separate, unresolved design question, not asserted by the test.
      This fix also turned up two further issues — see below.

- [x] `execute.rs`'s `source()` never checked `Scope::is_stopped()` between top-level
      statements in a script — `crush:exit`/`return`/`break` setting `is_stopped` only
      actually stopped the rest of a script *by accident*, because the job.rs bug above
      turned "the stopped statement produced no output" into a `RecvError` that
      propagated up through `source()`'s `?` and aborted its loop. Fixing job.rs above
      removed that accidental mechanism, regressing `tests/exit.crush` (which expects
      `echo 3` to never run after `crush:exit`) — `echo 3` started running again.
      Fixed: added an explicit `if global_env.is_stopped() { break; }` after each
      top-level job in `source()`'s loop. Verified directly with a bare `crush:exit`
      (no block) correctly stopping the script. `tests/exit.crush` itself was still red
      at the time for an unrelated, newly-exposed reason — see the next item, also now
      fixed — and is green again as of that fix.

- [x] `crush:exit`'s "are there other jobs running" check (`random_other_job()` in
      `src/builtins/crush.rs`) filtered only by `job.id != my_job_id` — it didn't
      recognize "this other job is my own enclosing block/closure, not a genuinely
      unrelated concurrent job." So `crush:exit` called from *inside* any block, closure,
      or function body always spuriously failed with `"There are running jobs."` (the
      enclosing block itself counted as "another job"), regardless of whether anything
      else was actually running — confirmed with a single, bare `{crush:exit; 2}` as the
      very first statement in an otherwise empty script. `tests/exit.crush` uses exactly
      this shape (`{crush:exit; 2}`) and was never actually testing "exit successfully
      stops the script" — it happened to produce the expected output only because the
      job.rs bug above *also* propagated this failure up and aborted the script for an
      unrelated reason, which looked identical to "exit worked."
      Fixed: `JobData`/`JobInfo` gained a `parent: Option<JobId>` field, set via a new
      `GlobalState::create_nested_job_handle()` / `JobContext::new_nested()` pair (used
      by `closure.rs`'s `eval_inner`, the only place a job is evaluated *inside* another
      one today); `random_other_job()` now walks the parent chain via a new
      `is_ancestor()` helper and excludes ancestors, not just self. Verified a genuinely
      unrelated concurrent job (`loop {} &`) is still correctly detected and still
      blocks a plain `crush:exit`. `tests/exit.crush` now passes for the right reason.
      Known follow-up, not covered by this fix: command substitutions (`$(...)`, via
      `value_definition.rs`'s `EvalContext`, which carries no job-handle info at all
      today) likely have the same underlying issue but go through a different,
      untouched path.

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

- [x] `control/while.rs` — now covered by `tests/while.crush`, including its documented
      "no body -> condition is the body" alternate mode.
- [x] `stream/group.rs` — only single-key, non-empty grouping was exercised. Covered by
      `tests/group.crush`: multi-column grouping, and grouping an empty stream (no crash
      or hang, just an empty output). Aggregator-command failure inside the spawned
      worker thread turned up a real, notable behavior worth documenting precisely:
      when an aggregator fails for one group's rows, that group's row is silently
      dropped from the output entirely — `group` itself doesn't fail, and other groups
      whose aggregation succeeds are emitted correctly and unaffected. The error is
      printed (along with a secondary, unrelated stray-channel-error message, the same
      class already fixed elsewhere in the codebase but not yet addressed here in
      group.rs's own internal worker-thread channels), but nothing about stdout or exit
      status indicates a group went missing. Captured as confirmed current behavior, not
      fixed — whether a failed group should instead make the whole `group` command fail
      is a real design question, not decided here.
- [x] `stream/uniq.rs` whole-row dedup (`field: None`, hashing an entire `Row` including
      floats) is now covered by `tests/uniq_whole_row.crush`. The "structs" half of this
      turned up a real, reachable panic — see the `Reachable panics` section below.
- [x] `types/re.rs` / `one_of.rs` — now covered by `tests/regex.crush`:
      match/not_match, replace vs replace_all (including capture group references in
      the replacement text), filter's per-column and error behavior, `re:new`'s
      invalid-pattern error, and `one_of` restricting a closure parameter's allowed
      types. No bugs found.
- [ ] `(expr)` -> synthetic `val` desugaring and `[...]` list-literal desugaring
      (`src/lang/ast/node.rs`) — this exact mechanism is what caused the real completion
      bug fixed earlier this session. `list_literal` uses a structurally similar
      synthetic-command trick and has no test for nested `(expr)` inside `[...]`, empty
      `[]`, or interaction between the two.
- [ ] `closure.rs` (1323 lines, largest file in the crate) has zero direct unit tests;
      only indirectly covered via `tests/closure_signatures.crush`.
