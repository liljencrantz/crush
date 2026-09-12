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

- [x] `Job::eval()` (`src/lang/job.rs`) only tracked and joined the thread for a
      pipeline's *last* command. Non-last stages' `call_def.eval(context.with_io(input,
      output))?;` return value (an `Option<ThreadId>`) was discarded outright, so if that
      stage was dispatched to a thread (can_block=true, the default for most builtins),
      nobody ever joined it — its result, success or failure, never reached anywhere that
      would report it. Found while making pipeline-step failures halt the script: a
      non-last `uniq` failing (in a `... | uniq | echo` pipe) still silently vanished
      even after fixing every swallow point in the dispatch chain (`eval_command`,
      `CommandInvocation::eval()`, `join_one()`), because nothing ever called
      `join_one()` for that stage's thread at all.

      **Fixed**, in two parts:
      1. `CrushError` gained `is_send_disconnected()` (`src/lang/errors.rs`), matching
         only `SendError` — crossbeam's `SendError` has exactly one meaning ("every
         receiver was dropped"), unlike the existing `is_disconnected()` (which also
         treats `RecvError` as benign, for the unrelated reason that `next_row()` uses
         it to mean ordinary end-of-stream — reusing that for this purpose would have
         also swallowed the exact class of bug `next_row()` exists to catch).
      2. `Job::eval()` now collects the `ThreadId`s it used to discard and, once the
         pipeline's last stage finishes, joins each one — propagating a genuine failure
         as the job's own, but ignoring one where `is_send_disconnected()` (e.g. a
         `head`/`take` truncating a stream early, which makes an upstream stage's next
         `output.send(row)?` fail exactly this way; every streaming command's existing
         `?`-propagation on that call was already correct, it just needed something to
         actually look at the result).

      This alone deadlocked `&`-backgrounded pipelines (`seq 100 | pipe:write &`):
      `&` desugared to a literal trailing `bg` pipeline stage whose entire purpose is to
      *not* wait for upstream to drain, and the new join now waited anyway. Rather than
      detecting "is the last stage `bg`" at runtime (tried and rejected: name matching
      breaks under `$bg := {}` shadowing; a `bg`-identity check via `Arc::ptr_eq` handles
      shadowing/aliasing correctly but needs the command position resolved twice, which
      is unsound if it ever has a side effect) — `background: bool` became a real,
      first-class field on `Job`/`JobNode`, set only by the grammar's existing
      `OptBackground` production (the same place that recognizes `&` today), and `bg` was
      split into two unrelated things: a job-level `is_background` flag that
      `Job::eval()` itself acts on directly (dispatch the real last stage, register its
      output receiver in `GlobalState`'s new `background_jobs` table for `fg`, return
      immediately — none of the new joining logic runs at all for a background job,
      matching "a background job is its own independent context, its intermediate
      commands' exit status is never checked, and nothing waits for its threads"), and a
      narrower `control:bg` builtin repurposed to resume an already-paused job into the
      background (parallel to `crush:resume`, not related to job creation at all
      anymore). `tests/bg_fg.crush`'s old `seq 0 3 | sum | bg` (the previously-documented
      "exactly equivalent to `&`" spelling) no longer works by design and was updated to
      use `&`; `pup` (de)serialization of a `Job` (`src/lang/command/closure.rs`, the
      `crush.proto` `Job` message) gained the same `is_background` field so this survives
      a serialization round trip (e.g. `remote:exec` of a closure containing a
      backgrounded job) instead of silently defaulting to `false`.
      Also fixed in passing: `control:bg`'s own doc example (`seq 100_000 | pipe:write
      &`) was itself wrong — `seq`'s first positional argument is `from`, not a count, so
      that example actually meant "count forever from 100,000," an infinite producer.
      Verified against the real repro (`tests/pipe.crush`, previously deadlocking) and
      against `head`/`take`-style early termination and genuine non-last-stage failures
      (`uniq` as a non-last stage now correctly aborts the script instead of vanishing)
      by hand.

      **New evidence for a related, still-open concern:** confirmed by hand that
      `try`/`catch` does *not* reliably see a value assigned or returned from a
      *previous* statement if the very next statement immediately does something with it
      (a method call, string concatenation) with no intervening statement — a fifth
      repro shape of a class already noted elsewhere in this file (see the
      thread-join-gap notes under "Test infrastructure gaps" and the
      "`remote.rs`" section below). Not investigated further here.

      **Root cause found** (after further digging prompted by the observation, below,
      that running `tests/pipe3.crush` under real OS-level parallelism reproduces it far
      more reliably than `cargo test` ever did): a genuine, confirmed race between
      `pipe:close` and any `pipe:read`/`pipe:write` invocation that was issued (as a
      background job, e.g. `pipe:read | sum &`) but whose resolution thread hadn't
      actually started running yet when `pipe:close` executes.

      `pipe:read`/`pipe:write` are plain struct-member values (`$pipe`'s `read`/`output`
      fields), not `Value::Command`s, so `CommandInvocation::can_block()`'s catch-all
      (`_ => true` for anything that isn't a resolved `Value::Command`) makes *evaluating
      them* — i.e. the `pipe.get("read")`/`pipe.get("output")` field lookup itself, not
      just any actual streaming work — happen on a freshly spawned worker thread rather
      than synchronously on the thread creating the job. `pipe:close` (`close()` in
      `src/builtins/types/table_input_stream.rs`) clears `$pipe`'s `read`/`output` fields
      to `Value::Empty` as soon as it runs, with no synchronization against any
      already-dispatched-but-not-yet-running reader/writer thread. When a reader/writer's
      worker thread happens to get scheduled by the OS *after* `pipe:close` already ran
      (a real possibility any time `pipe:close` is the very next statement, as it is in
      `tests/pipe3.crush` right after the 4th `pipe:read | sum &`), that thread reads
      `Value::Empty` instead of the real stream handle, sends `Value::Empty` onward, and
      the downstream command (`sum`) fails immediately trying to treat it as a stream —
      which is exactly the observed "receiving on an empty and disconnected channel"
      (the failed `sum` never reaches its own `context.output.send(...)`, so whoever
      later does `fg` on that job sees its sender dropped) and, in the `pipe:write` case,
      an undercounted sum (assertion failure) since that writer never wrote any of its
      100,000 rows. The pipe docs' own claim that `pipe:close` "does not interrupt
      currently existing read or write jobs" is true only if "currently existing" is
      read as "already scheduled and running" rather than "already issued in program
      order" — the latter is what a user would reasonably expect from the example in
      `pipe:pipe`'s own docs, and what actually breaks here.

      Confirmed, not just theorized: 30 parallel instances of `tests/pipe3.crush` against
      the post-`Job::eval()`-fix binary reliably fail 50-60% of the time (12/30 and
      17-18/30 hung or failed across two runs), and the failing `sum` invocation is
      *always* one of the two last-created reader jobs (`job_id` 8 or 9 out of the four
      reader jobs 6-9, i.e. `ThreadId` 24 or 26 specifically) in every one of 6 failing
      instances sampled with tracing — exactly the readers closest in program order to
      `pipe:close`. Disabling `pipe:close`'s two `pipe.set(..., Value::Empty)` calls
      (an experiment, not a real fix — it would leak the pipe's channel clones forever)
      took the same 30-parallel stress test from ~50-60% failures to 20/20 clean passes.
      That is direct causal confirmation, not correlation.

      This also explains why the earlier hangs (a hard deadlock at process exit, not
      just a printed error) only ever showed up on the post-fix binary: a `sum` thread
      that dies immediately on a bad input never blocks anything by itself, but the
      *other*, unaffected reader/writer threads for the same run can still be genuinely
      mid-flight when the top-level script's error already got printed, and
      `main.rs`'s final `global_state.threads().join(printer)` (present unchanged since
      before this session's `Job::eval()` work) blocks on all of them sequentially at
      shutdown — so any of those still-running threads blocked on a shared, only
      partially-drained pipe is enough to hang the whole process.

      Why does this race only bite the *new* `is_background` code path and not the old
      `& → trailing bg` desugaring, when the underlying vulnerability (deferred struct
      field lookup on a worker thread, racing a later `pipe:close`) is identical in both
      and doesn't depend on `Job::eval()`'s design at all? Confirmed empirically, not
      just argued: 100 total parallel runs of `tests/pipe3.crush` against the pre-fix
      binary (commit `a4a8189`, two batches of 30 and 40) came back 100/100 clean, at the
      same or higher parallelism than the post-fix binary's ~50-60% failure rate. The
      likely explanation is pure timing, not a different bug: the old `bg` builtin's own
      dispatch (a full `CommandInvocation` invocation — argument-struct parsing via the
      `#[signature]` machinery, a real command lookup — before it finally calls
      `add_job`/sends its output) is measurably slower than the new `is_background`
      branch's direct field access and `Vec` push, and that extra latency was
      apparently enough of a head start for the OS scheduler to get a job's
      `pipe:read`/`pipe:write` worker thread running before the *next* top-level
      statement's (`pipe:close`'s) own thread got scheduled. The redesign didn't
      introduce a new bug so much as shave away the accidental delay that had been
      hiding a pre-existing one. This was NOT re-verified by re-introducing an artificial
      delay into the new code path and confirming the failure rate drops back down —
      that would be the next thing to try if this timing explanation itself needs
      firming up.

      **Fixed — but not the way first attempted.** Two runtime-level candidate
      directions were considered: (a) make `pipe:close` wait for any reader/writer job
      issued before it to actually resolve its field reference before clearing, or (b)
      make the `$pipe`-struct-member field lookup for `pipe:read`/`pipe:write` resolve
      *synchronously* at dispatch time instead of being deferred into a worker thread,
      by fixing `ValueDefinition::can_block()`'s `GetAttr` case (previously unconditional
      `true`, regardless of what the parent expression resolves to) to instead delegate
      to `inner.can_block(context)`.

      (b) was actually implemented and tried. It fully fixed the *read* side — 30
      parallel `tests/pipe3.crush` runs went from ~50-60% hangs/failures to 0/30 hangs
      and 0/30 "disconnected channel" errors. But it made the *write* side measurably
      worse (26/30 wrong-sum failures, each short by an exact multiple of one writer's
      full 100,000-row contribution — never a partial amount). Root cause of that new
      failure: `pipe:write`'s own function body (`table_input_stream.rs`'s `write()`)
      does a *second*, independent field lookup (`pipe.get("output")`) on its own
      worker thread — a separate instance of the identical race, but inside the
      command's own implementation rather than in `CommandInvocation`'s dispatch logic,
      so the `GetAttr` fix can't reach it. Worse, making command *resolution*
      synchronous sped up everything else in the script, shrinking the writer threads'
      remaining head start over `pipe:close` even further, so they lost their own race
      *more* often, not less. This `GetAttr::can_block()` change was not landed, since
      the actual fix (below) needs no runtime change at all.

      **Actual fix: `fg` every writer job before calling `pipe:close`, on the
      unmodified pre-existing runtime.** `fg`'s recv() only returns once a job's last
      stage has *entirely finished* and sent its own output value — for a writer job
      (`seq ... | pipe:write &`), that only happens after `write()`'s body has
      completely drained its input and returned, meaning it has already safely used
      `$pipe`'s "output" field. `fg`-ing every writer job before `pipe:close` therefore
      *deterministically* eliminates the write-side race (not just makes it unlikely):
      by the time `pipe:close` runs, every writer has provably already finished reading
      that field. This doesn't equally deterministically protect the read side (a
      reader's field-lookup thread getting scheduled isn't logically caused by waiting
      on writers) — but a field lookup is astronomically cheaper than draining
      hundreds of thousands of rows, so in practice the reader threads get scheduled
      long before the last writer finishes. Confirmed empirically on the completely
      unmodified binary (no source changes): 70/70 parallel `tests/pipe3.crush` runs
      clean across two batches (30 and 40), and 25/25 each for both `tests/pipe2.crush`
      and `tests/pipe3.crush` run concurrently (50 total), plus three full
      `cargo test --workspace -- --test-threads=1` runs with zero failures.

      Applied to `tests/pipe2.crush` and `tests/pipe3.crush` (each writer job now
      explicitly captured and `fg`-ed before `pipe:close`; `pipe2.crush`'s single
      writer, previously a bare unnamed `&` statement, had to be assigned to a variable
      first so it could be `fg`-ed at all) and to `pipe:pipe`'s own doc example in
      `table_input_stream.rs` (which had the identical racy shape — create writer,
      create reader, close immediately — meaning a user following the documented
      idiom verbatim would have hit this). Also fixed the doc example's `seq 100_000`
      (an infinite producer, not "100,000 integers" — see the `Job::eval()` entry
      above for the same incidental bug found elsewhere) to `seq 0 100_000`.
- [ ] `control/schedule.rs` has a genuine, pre-existing race in its own output-channel
      lifecycle: when its result is left as a bare, unconsumed top-level statement (no
      pipe, no assignment), the row it sends via `output.send(Row::new(vec![]))` (or the
      equivalent for the `command=` variant) races against its own
      `initialize_output`-sent handshake sitting unread, and fails with "sending on a
      disconnected channel". This was always happening — confirmed by hand — but was
      silently swallowed by `join_one()`'s old discard-everything behavior, so it never
      surfaced as a real error before pipeline/closure error propagation was fixed (see
      the two entries above this one). Worked around in `tests/schedule.crush` by piping
      the `command=` case to `| echo` instead of leaving it bare, which avoids
      triggering the race, rather than fixing schedule.rs's own channel handling.
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
      to `while let Some(row) = ... .next_row()? { }`. Originally covered only by a
      custom Rust assertion on stderr (`schedule` is a non-last pipeline stage here,
      piped to `echo`, and at the time `Job::eval()` never joined a non-last stage's
      thread, so its error could never become a catchable/observable `CrushResult::Err`
      and there was no synchronous point to hang a stdout marker on). Converted to a
      plain stdout-diff test, `tests/schedule_does_not_leak_a_stray_channel_error_on_exhausted_input.crush`,
      once the `Job::eval()` thread-join-gap entry below was fixed: a regression now
      genuinely aborts the script (confirmed by hand by temporarily reintroducing the
      old bug), so an `echo "reached"` marker after the pipeline has a real failure
      mode again. The pipeline's own row output is captured into a variable via `count`
      rather than left to print to the terminal — printing a stream goes through
      crush's asynchronous background pretty-printer, which isn't ordered against the
      next statement's own output, so leaving it printing directly made the marker's
      position in stdout racy against unrelated printer-thread timing, observed
      directly (one stray reordered run out of several dozen) before switching to
      `count`. Confirmed red before the original fix, green after.
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

- [x] `Value::param_partial_cmp` (`src/lang/value/mod.rs`) had no match arm for
      `(Value::Type(val1), Value::Type(val2))`, so it fell through to the catch-all
      `_ => None` — the same "incomparable" result `f64::partial_cmp` gives for NaN.
      `Value::Table`'s `PartialEq` (and `Struct`'s, and `List`'s in `Regular` mode) is
      implemented via `partial_cmp(...) == Some(Equal)`, not a direct recursive `eq()`,
      so this meant any `Table`/`Struct`/`List` containing a `Type`-valued cell never
      compared equal to anything — including itself. Invisible until now because nothing
      previously produced `Type` values as *stream/table data* rather than a bare
      top-level value (where `Value`'s own `PartialEq::eq()` — a separate, direct match
      that does handle `Type` — is what actually gets used). Surfaced immediately by the
      new `members` builtin, whose entire output is `type`-valued cells: `$m == $m` was
      `$false` for its own materialized output. Fixed by adding
      `(Value::Type(val1), Value::Type(val2)) => Some(val1.cmp(val2))`. Covered
      implicitly by every `assert (... == ...)` in `tests/members.crush` and
      `tests/fs_watch.crush`'s schema check, both of which compare `members`' output
      tables directly and would fail immediately without the fix.
- [x] Struct field access (`$s:fieldname`) broke when the field's value was a `File` —
      reproduced with a plain `struct:of somefile=./x kind="y"` followed by `$s:somefile`:
      no panic message, just "receiving on an empty and disconnected channel". Root
      cause: `eval_command_definition` (`src/lang/command_invocation.rs`, added by
      `ea58c71` "When a path is given as the command, execute it") treated *any*
      zero-argument expression that *resolved* to a `Value::File` as something to `cd`
      into or execute — not only a File literal genuinely written at the head of a job
      (`./foo`). Member access (`$s:x`) is evaluated through the same function with
      `this` bound and zero arguments, so a `File`-valued field got silently run as a
      subprocess (confirmed directly: pointing the field at a real executable made it
      actually execute and its stdout come back as the "result"). Fixed by checking the
      *unevaluated* command position, not just the resolved value: only
      `ValueDefinition::Identifier`/`Value` (a bareword or literal path token) may
      trigger execution now; anything else that merely evaluates to a File (`GetAttr`,
      i.e. member access, and by extension anything else that might resolve to one)
      falls through to plain passthrough, matching every other value type. Deliberately
      an allow-list, not a deny-list on `GetAttr` specifically, so a future
      `ValueDefinition` variant fails closed (never executes) by default. Covered by
      `tests/file_value_member_access.crush` (confirmed red before the fix, green
      after), which also guards the original feature (`./foo` still executes when
      genuinely written at a job's head). Originally found while writing
      `tests/fs_watch.crush` (whose rows have a `File`-typed `path` column), which still
      reads its columns via `select`/`list:collect` rather than materializing a row and
      indexing into it directly — that workaround wasn't reverted, since it's unrelated
      to what that test is actually meant to cover.
- [ ] `!~` (the negated-match expression-mode operator) does not parse at all —
      `assert (fooo !~ ^(zzz))` fails with `Unrecognized token '!' ... Expected one of
      LogicalOperator, MemberOperator, ...`, even standalone. Confirmed pre-existing:
      reproduces identically on the unmodified binary from before this session's
      `like`/`match` work, which only touched `!~`'s *semantic* desugaring (the string
      passed to `operator_method`, now `"not_like"` instead of `"not_match"`) — the
      failure is a lexer/parser-level token recognition problem, upstream of anything
      changed here. `=~` (the positive form) parses and works correctly. Root cause not
      investigated.
- [ ] A glob literal on the right-hand side of `=~` inside `(...)` expression mode does
      not parse — `assert (foo.txt =~ *.txt)` fails with `Unrecognized token '*' ...`.
      Also confirmed pre-existing on the unmodified binary. `docs/overview.md`'s own
      documented example for this (`crush# foo.txt =~ *.txt`) is apparently only ever
      exercised at the bare interactive-prompt level, never inside `(...)`/`assert`, so
      this was never caught by existing tests. Root cause not investigated.

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
      per-row. Covered by `tests/uniq_does_not_panic_on_unhashable_type.crush`, kept as
      its own short, isolated script rather than folding it into a larger combined test
      (originally because a panicking thread and a graceful error both leave stdout
      empty and whether the panic message reaches stderr before the whole process exits
      turned out to be a race in a longer script, confirmed by hand; now a pure crush
      `try`/`catch` test — see the "Test infrastructure gaps" section below for how and
      why that conversion became possible).
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
- [x] `stream/each.rs:45` panicked unconditionally on essentially every normal
      invocation — "index out of bounds: the len is 0 but the index is 0". Cause:
      `each()` called `Each::parse(context.remove_arguments(), ...)` and *then* read
      `context.arguments[0].source` on the next line, but `remove_arguments()`
      (`src/lang/state/contexts.rs`) empties `context.arguments` via `mem::swap` before
      returning the removed values, so that second line always indexed into an
      already-empty vec. `where.rs`'s `r#where()` has the same two operations in the
      opposite (correct) order — clone the source out first, then call
      `remove_arguments()`. Never caught before because no existing `tests/*.crush` file
      actually invoked `each` as the stream command (`grep -rl '\beach\b' tests/*.crush`
      only matched the English word inside comments). Fixed by reordering each.rs to
      match where.rs's pattern. Covered by `tests/warnings.crush`'s new each.rs section
      (added while retrofitting each.rs to the warning log below), which would have hit
      this panic on its very first `each` invocation before the fix.
- [ ] `types/integer.rs:117` (integer `/`) panics with "attempt to divide by zero"
      instead of returning a `CrushResult::Err`, the same bug class as the
      already-fixed `stream/aggregation.rs` avg-of-empty-stream panic above. Found
      incidentally while probing for an `each.rs`/`where.rs` warning-retrofit repro
      (`10 / $value` where `$value` could be `0`); not yet fixed or covered by a test.

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
      `tests/last_command_error_does_not_leak_a_stray_channel_error.crush` (a pure crush
      `try`/`catch` test, auto-discovered by `test_finder!()` — see the "Test
      infrastructure gaps" section below) — now passing. Deliberately out of scope:
      whether a failing last command should also make the process exit non-zero
      (currently exits 0) — punted as a separate, unresolved design question, not
      asserted by the test.
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

- [x] `remote.rs` SSH host-key verification (`exec`/`pexec`, ships a serialized closure
      over SSH and deserializes the result). This entry was stale — the SSH-server test
      work elsewhere this session (`ssh-service/`, `tests/system.rs::test_remote_ssh`)
      had already covered `known_hosts.check_port`'s `Match` (`ssh_exec.crush`),
      `Mismatch` (`ssh_exec_mismatch.crush`, a corrupted-but-still-valid-base64 key),
      `NotFound` with `allow_not_found=false`/default (`ssh_exec_notfound.crush`), and
      `NotFound` with `allow_not_found=true`'s TOFU auto-add path
      (`ssh_exec_allow_not_found.crush`, which also confirms the real key actually gets
      written back to the known_hosts file). The one genuinely missing case,
      `ignore_host_file=true` (skip verification entirely), is now covered by
      `tests/remote/ssh_exec_ignore_host_file.crush`: it points at the *same corrupted*
      known_hosts file used by the mismatch test and asserts the connection succeeds
      anyway — since that file would otherwise cause a hard `Mismatch` error, success
      here can only mean the check was actually skipped, not coincidentally passed.
      `CheckResult::Failure` (a genuine libssh2-level validation failure, distinct from
      `Mismatch`) remains untested — not obviously reachable without deeper key-format
      manipulation than a test is worth here.
- [ ] `dbus.rs` (843 lines) and `systemd.rs` — Linux-only, zero coverage on any platform
      (can't even be exercised in CI on macOS/most dev machines).
- [ ] `dns.rs` — real UDP queries with response parsing against attacker-influenceable
      input (reverse-DNS lookups); response *parsing* itself is entirely delegated to
      the `trust_dns_client` crate, so no malformed/truncated-response handling is
      verified here (deliberately not adding tests for that -- it would just be
      re-testing the third-party library, not crush's own code). Fixed in passing: a
      concrete, crush-specific issue found while reading `perform_query` for this
      entry -- CNAME-chasing had no depth limit or cycle detection, so a malicious,
      compromised, or spoofed nameserver (plain UDP DNS has no cryptographic
      integrity) returning a self-referencing CNAME chain would recurse forever, one
      fresh network round trip per hop. Added a hard-coded `MAX_CNAME_DEPTH = 8` cap,
      threaded through `query_internal`/`perform_query` as an explicit `depth`
      parameter, erroring out past the limit instead of recursing further.

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
      the shorter of the two — since fixed with an added length check. Two dedicated
      tests proving that fix worked, `test_run_system_test_catches_missing_trailing_lines`/
      `..._extra_trailing_lines` and their `tests/harness/too_{few,many}_lines.crush{,.output}`
      fixtures, were added at the time and later dropped — testing the test harness
      itself was judged too meta to be worth the maintenance cost of two dedicated
      tests, given `run_system_test` is small, foundational, and rarely touched.): `zip $(lines:from
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

- [x] Audited every custom-Rust-assertion test in `tests/system.rs` (as opposed to a
      plain `.crush`/`.crush.output` golden pair) to see which could become pure crush
      tests, per standing preference. Two of five converted:
      `test_last_command_error_does_not_leak_a_stray_channel_error` and
      `test_uniq_does_not_panic_on_unhashable_type` now wrap their repro in `try`/`catch`
      and assert on the caught message directly inside the `.crush` file. Once that
      meant the Rust side only needed the default exit-code-0 check, there was no
      remaining reason for either to be a bespoke `#[test]` fn at all — both moved from
      `tests/error_handling/` (deliberately not auto-discovered) to the top level as
      `tests/last_command_error_does_not_leak_a_stray_channel_error.crush` and
      `tests/uniq_does_not_panic_on_unhashable_type.crush`, named to match their old
      Rust function names exactly so `test_finder!()`'s auto-generated `test_<filename>`
      preserves the same descriptive test names with zero custom Rust code left at all.
      Both were previously custom Rust tests because the code path they exercised, at
      the time they were written, printed the
      real error via a fire-and-forget path (`command_invocation.rs`'s old
      `handle_error()`) rather than a catchable `CrushResult::Err` — that comment is now
      stale, since `eval_command`'s non-blocking path no longer calls `handle_error()` at
      all (a `?` propagates directly). Confirmed empirically, not just from reading the
      diff: a genuine, still-uncorrected panic elsewhere in the codebase (integer
      division by zero, see the Reachable-panics section above) is *not* silently
      swallowed by `try`/`catch` — it still surfaces (as raw panic text plus a leaked
      channel error), so a regression reintroducing either bug would still turn these
      tests red.
      `test_schedule_does_not_leak_a_stray_channel_error_on_exhausted_input` was **not**
      convertible at the time this audit was done: its error happens on `schedule`'s own
      spawned thread, and since `schedule` is a non-last pipeline stage in that repro
      (piped to `echo`), the thread was never joined by `Job::eval` (the thread-join-gap
      entry above, unfixed at the time), so it could never become a catchable `Err`
      either way — confirmed by hand, wrapping the exact script in `try`/`catch` never
      entered the `catch` block regardless of whether the underlying bug was present.
      **This is now stale**: once the thread-join-gap entry above was fixed, this test
      *was* converted — see that entry for the details (it ended up using a plain
      stdout marker, not `try`/`catch`, since the goal was proving the script doesn't
      abort at all, not inspecting an error's content). `test_run_system_test_catches_missing_trailing_lines` and
      `test_run_system_test_catches_extra_trailing_lines` were left for a different
      reason: they test `run_system_test`'s own comparison logic (via
      `std::panic::catch_unwind`), not crush language behavior — there's no crush-level
      equivalent to "assert this Rust function panics." (Both since dropped entirely,
      along with their `tests/harness/` fixtures — see the `test_zip` entry above.)
      **New evidence for the existing `Job::eval()` thread-join-gap entry, a fifth repro
      shape:** touching a value assigned inside a `catch` block — a method call or
      string concatenation — in the very next statement after the `try`/`catch` reliably
      raced and surfaced the same leaked "disconnected channel" error, independent of
      the tests' own subject matter. A single barrier statement (e.g. a plain `echo`)
      between the `try`/`catch` and the first use of the caught value reliably avoided
      it in every case tried. Not investigated further or fixed; both converted `.crush`
      files include one deliberately, with a comment explaining why.

## Untested control-flow / stream ops (lower severity, still real gaps)

- [x] `control/while.rs` — now covered by `tests/while.crush`, including its documented
      "no body -> condition is the body" alternate mode.
- [x] `stream/group.rs` — only single-key, non-empty grouping was exercised. Covered by
      `tests/group.crush`: multi-column grouping, and grouping an empty stream (no crash
      or hang, just an empty output). Aggregator-command failure inside the spawned
      worker thread turned up a real, notable behavior worth documenting precisely:
      when an aggregator fails for one group's rows, that group's row is silently
      dropped from the output entirely — `group` itself doesn't fail, and other groups
      whose aggregation succeeds are emitted correctly and unaffected. Nothing about
      stdout or exit status indicates a group went missing, and whether a failed group
      should instead make the whole `group` command fail is still a real, undecided
      design question — but the failure is no longer *invisible*: `group.rs`'s two
      swallow points (the per-column aggregator failure and the group-collection
      failure) now both report through the new warning log
      (`GlobalState::warn`/`crush:warnings`, see `tests/warnings.crush`) instead of just
      printing via `Printer::handle_error`, so a script can check afterward whether any
      groups were dropped.
- [x] `stream/uniq.rs` whole-row dedup (`field: None`, hashing an entire `Row` including
      floats) is now covered by `tests/uniq_whole_row.crush`. The "structs" half of this
      turned up a real, reachable panic — see the `Reachable panics` section below.
- [x] `types/re.rs` / `one_of.rs` — now covered by `tests/regex.crush`:
      match/not_match, replace vs replace_all (including capture group references in
      the replacement text), filter's per-column and error behavior, `re:new`'s
      invalid-pattern error, and `one_of` restricting a closure parameter's allowed
      types. No bugs found.
- [x] `(expr)` -> synthetic `val` desugaring and `[...]` list-literal desugaring
      (`src/lang/ast/node.rs`) now covered by `tests/math_mode_expressions.crush`:
      plain `(expr)`/`[...]`, nested `(expr)` inside `[...]`, a list literal nested
      inside another, a list element that's a command substitution, and empty `[]`.
      `[]` turned out to be a real, notable finding worth documenting: it doesn't
      produce an empty list — `[...]` always desugars to `list:of`, which needs at
      least one argument to infer the element type from, so `[]` errors.
- [x] `closure.rs` (1323 lines, largest file in the crate) now covered by
      `tests/closures.crush`: variable capture, default parameter values, repeated
      named arguments collecting into a list, `@$rest`/`@@$rest` collectors, and
      `return`'s bare-block restriction. No bugs found.
- [x] Extended the warning-log retrofit (see `stream/group.rs` above) to the other
      swallow points that were identified when the warning log was first designed, plus
      a fresh audit of the rest of the builtins for the same pattern. `stream/where.rs`
      (a failing predicate) and `stream/each.rs` (a failing body, once its own indexing
      panic above was fixed) now report per-row failures via `global_state.warn(&e)`
      instead of just printing, exactly like `group.rs`/`fs:files`. Also retrofitted:
      `types/mod.rs`'s `new()` (a `class()`-based object's `__init__` constructor
      failing no longer just prints — the object is still returned, partially
      initialized, matching prior behavior, but the failure is now visible via
      `crush:warnings`) and `grpc/client.rs`'s `invoke_method` (a streaming RPC response
      row that fails to convert). Unlike `fs:files`, none of these four needed a bespoke
      command/source-attaching helper: their errors all originate from evaluating a
      user-supplied `Command`/closure (`condition.eval(...)`, `c.eval(...)`), which goes
      through `Closure::eval()`'s normal `.with_command(self.name())` /
      `.with_source_fallback(&source)` enrichment automatically — confirmed live: an
      anonymous closure body's warning records `command == "<block>"`
      (`ClosureType::Block`'s `name()`), and `__init__`'s records `command ==
      "__init__"`. Covered by three new sections in `tests/warnings.crush` (where/each/
      class-init), extending the existing group/files/warning_limit coverage.
      Deliberately **not** converted, with reasoning: `stream/join.rs:50`
      (`printer.handle_error(output.send(...))`) is output-channel-closed handling, not
      a data-quality partial failure — warning on it would spam once the pipe is
      already broken. `control/timeit.rs:49` is about an internal output-draining
      helper thread's own failure, not the timed closure's result (which already
      propagates via `?`) — a much weaker candidate than the other four. `users.rs`'s
      and `control/cmd.rs`'s `printer.error(err)` calls are both subprocess-stderr-relay
      loops (sudo and `cmd`, respectively), printing arbitrary external-process text
      lines — philosophically different from a structured per-item command warning.
      Various discarded `let _ = ...send(...)` results in `group.rs`/`io/csv.rs`/
      `fs/watch.rs` are channel-lifecycle noise (receiver disconnected), not
      data-quality issues.
      **New evidence for the existing `Job::eval()` non-last-pipeline-stage
      thread-join-gap entry above:** writing this coverage surfaced the same race from a
      new angle. Two consecutive `can_block=true` top-level jobs run back-to-back with
      no intervening statement (e.g. an `each` invocation immediately followed by a
      `class():new` call, with nothing between them) reliably raced a *subsequent*
      `crush:warnings | materialize | select ... | list:collect` read with "receiving
      on an empty and disconnected channel" — reproduced deterministically (not flaky)
      across several minimal variants; inserting any cheap top-level statement (an
      `echo`, or a `crush:warnings | materialize | count` check) immediately after each
      trigger reliably avoided it. `tests/warnings.crush`'s new sections use exactly
      that pattern (an immediate `count`-based `assert` after every triggering
      statement) deliberately, not just for readability. Not investigated further or
      fixed — same underlying class as the existing entry, just a second, easier
      repro shape.

## `remote.rs` (SSH remote execution) — previously untested, now covered

- [x] `src/builtins/remote.rs` had zero test coverage before this. Three layers added:
      1. `parse()` (host/user/port splitting) is a pure function with no I/O — covered
         by six `#[cfg(test)]` unit tests directly in `remote.rs` (`user@host`, explicit
         port, default-username argument, falling back to `get_current_username()`,
         an invalid port, and `user@host:port` together). No bugs found.
      2. `remote:host:list`/`remote:host:remove` never open a connection at all
         (`Session::new()` with no `.handshake()`) — they only read/rewrite a
         known_hosts file — so they're covered by a plain golden test
         (`tests/remote/host_list_remove.crush`) against a static fixture (three
         throwaway public keys, generated once with `ssh-keygen` and never used to
         authenticate anywhere — see `test_remote_host_file` in `tests/system.rs`,
         which writes the fixture to a fresh temp file every run since `host:remove`
         mutates its known_hosts file in place). One real footgun found, not a bug:
         `remote.host.remove`'s `key` filter (a `Patterns` value) matches only against
         the raw base64 key blob, not an "algo base64" pair, and needs a `**` glob (not
         `*`) to match it, since `crate::util::glob::Glob`'s `*` doesn't cross `/`
         boundaries and base64-encoded key blobs routinely contain `/`. A bare `key=*`
         silently matches nothing. Not changed (matches the same `*`-vs-`**`
         path-glob convention used for file globs elsewhere), just documented.
      3. `remote:exec`/`remote:pexec`'s actual SSH wire behavior — the thing that
         actually matters (host key checking, auth, exec/read/write) — needed a real
         SSH server. Added `ssh-service/`, a new sibling crate to `grpc-service/`
         following the exact same pattern (`escargot`-built, spawned as a subprocess by
         `tests/system.rs`'s `test_remote_ssh`): a minimal `russh`-based server that,
         on every accepted exec channel, spawns the real local `crush --pup` binary and
         pipes the SSH channel's data straight to/from that child process's stdin/
         stdout. This means the test exercises a genuine pup wire round trip through
         crush's own client code (`ssh2`-based), not a reimplementation of the
         protocol inside the test server. It generates a fresh Ed25519 host key every
         run and prints its known_hosts-format line on startup, so `test_remote_ssh`
         can build matching/mismatched/empty known_hosts fixtures without either side
         hardcoding key material. Covers, via `tests/remote/ssh_exec*.crush`: the happy
         path (`remote:exec` and `remote:pexec`, the latter against two hosts in
         parallel), a host-key mismatch (`CheckResult::Mismatch`), a host missing from
         known_hosts without `allow_not_found` (`CheckResult::NotFound`, must error),
         the same case *with* `allow_not_found` (must succeed and pin the key into the
         file — checked on the Rust side by re-reading the file afterward), and a wrong
         password. No bugs found in `remote.rs` itself; host-key checking already
         behaves exactly as documented in every branch tested.
      **New evidence for the existing `Job::eval()` non-last-pipeline-stage
      thread-join-gap entry above, a third repro shape:** capturing a `can_block`
      command's result (`$n := $(remote:host:remove ...)`) and then immediately calling
      a *method* on it (`$n:to_string`) in the very next statement reliably hit the same
      masked "receiving on an empty and disconnected channel" error in place of the real
      result — even though the capture itself succeeded (a plain `echo $n` right after,
      with no method call, correctly printed the real value). Comparing the captured
      value directly in a numeric `assert` (`assert ($n == 1) "..."`, no `:to_string`,
      matching `tests/warnings.crush`'s existing style) reliably avoided it. Not
      investigated further or fixed; all new `.crush` tests in `tests/remote/` were
      written around this rather than tripping over it.
      **Deliberately out of scope:** `remote:identity` (lists ssh-agent identities) was
      not covered — it needs a real running `ssh-agent` with a loaded key, which is an
      OS-level fixture outside what `ssh-service`'s test server can provide, in the same
      category as the already-noted "can't easily automate job-control signals"
      Reachable-panics entry above. Agent authentication (the `userauth_agent` branch of
      `run_remote`, used when no `password` is given) is untested for the same reason —
      only the `password`-set branch is exercised.
