# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

Crush is a command line shell that is also a modern programming language — closures,
lexical scoping, a type system, and a structured data pipeline (like a shell where
pipes carry typed tables/rows instead of bytes), written in Rust.

## Build / test / run

```
cargo build --release && cargo install --path .   # build + install to ~/.cargo/bin
cargo build                                        # debug build (produces ./target/debug/crush, used by tests)
cargo run                                          # run interactively
cargo test                                         # run all tests (see below)
```

OS deps must be installed before building (protobuf, openssl/libssl-dev, and on Linux
libdbus-1-dev + libsystemd-dev) — see README.md for the exact per-OS package lists.

### Tests

`cargo test` covers two very different things:

- Normal Rust unit tests scattered across the crates as usual (`#[test]` fns).
- **System/golden tests** under `tests/`: each `tests/<name>.crush` script is run against
  the *debug* binary (`./target/debug/crush`) and its stdout is compared line-by-line
  against `tests/<name>.crush.output`. These are auto-discovered by the `test_finder!()`
  macro (from the `test_finder` proc-macro crate) in `tests/system.rs` — dropping a new
  `foo.crush` + `foo.crush.output` pair into `tests/` is enough to add a new case, no
  Rust code needed. **Because these tests exec the debug binary, `cargo build` must be
  run (or be up to date) before `cargo test` picks up code changes** — `cargo test` alone
  does not rebuild the binary the system tests invoke.
- `test_grpc` builds and spawns the `grpc-service` sub-crate binary to exercise the gRPC
  client builtins.

To add/update a golden test: write the `.crush` script, run it manually through the
debug binary, and save the output as the matching `.crush.output` file.

Prefer this plain golden-test style over ad-hoc Rust assertions in `tests/system.rs`
(stderr/exit-code checks, etc.) — even for a bug whose only visible symptom seems to
need something other than a stdout diff (e.g. an async background thread's error). Two
techniques cover most such cases:

- If the only symptom is "the script aborts" (no message content to check): a
  top-level job's error aborts the rest of the script (see `execute.rs`'s `source()`),
  so performing the fragile operation and then `echo`ing a marker afterward turns "does
  this error?" into a plain stdout diff — the marker only appears if it didn't.
- If the error *message* itself matters: `try { <fragile op> } catch { |$e| $msg = $e }`
  captures the error as a plain string, which ordinary `assert`/`:like` calls can then
  check.

Only fall back to a Rust assertion in `tests/system.rs` when both are genuinely
impossible — and even then, prefer testing a structural property (e.g. "stderr is
non-empty") over coupling to exact error-message wording.

When a bug is found while working on something else, write its `.crush`/`.crush.output`
repro pair immediately, even before it's fixed — a known-failing test documents the bug
precisely and won't get lost or need rediscovering later.

## Crush language notes for script authors

A few non-obvious behaviors worth knowing before writing `.crush` test scripts or
tooling (e.g. `generate_docs.crush`) — the full semantics belong in
`docs/language_reference.md`, this is just the pitfalls that have cost real time:

- `:like`/`:not_like` do exact string match by design, not substring or wildcard
  matching — wildcarding is a property of a `glob` *value* passed as the pattern, not
  something the method name implies.
- Every codec's `:to` command (`json:to`, `yaml:to`, `pup:to`, ...) intentionally
  produces a `binary_stream`, not a plain string — this is deliberate ("bytes ready to
  write to disk or pipe onward"), not a quirk of any one codec. Use
  `convert $string $(...)` when a plain string is actually needed.
- String concatenation is not `+` (that's numeric/duration addition) — use `:format`.
- A regex literal's outer `^(...)` is Crush's own delimiter syntax, not itself capture
  group 1 — `^((.*)-(.*))` numbers its groups 1 and 2, not 2 and 3.

## Workspace layout

Cargo workspace with the main `crush` crate plus four local crates:

- `signature/` — proc-macro crate providing `#[signature(...)]`, the attribute used to
  declare every builtin command's argument struct (see below). This is the main piece
  of "framework" code in the project. Its generated code references `crate::lang::*`
  paths that only resolve inside the `crush` binary crate (which has no `[lib]` target,
  so `signature` can't depend on it), so a real macro expansion can only be compiled as
  part of `crush` itself — a `trybuild`-style compile-fail test in `signature/` can't
  isolate "rejected for the intended reason" from "failed because `crate::lang` doesn't
  exist here" without exact-stderr matching. `signature/src/lib.rs`'s own tests instead
  call `signature_real()` (the plain function the `#[proc_macro_attribute]` wrapper
  delegates to) directly and check `Result::is_err()`/`is_ok()` — see that file for the
  pattern if adding more macro-validation tests.
- `ordered_map/` — small insertion-ordered map type used by `OrderedStringMap` and struct values.
- `test_finder/` — proc-macro crate providing `test_finder!()`, which discovers the
  `tests/*.crush` golden files described above.
- `grpc-service/` — standalone example/test gRPC server binary used by `test_grpc`.

## Architecture

### `src/lang` — the language runtime

- `ast/` — lexer (`lexer.rs`) and AST node types (`node.rs`); `lalrparser.lalrpop` is the
  LALRPOP grammar, compiled by `build.rs` at build time into the actual parser.
- `parser.rs` / `execute.rs` — turn source text into jobs and run them.
- `job.rs`, `job_control.rs` — a *job* is a pipeline of commands; job control handles
  backgrounding/foregrounding, similar in spirit to a POSIX shell's job control.
- `command/` — `CrushCommand`/closure representation: what gets invoked.
- `command_invocation.rs` — resolves a parsed invocation to either a builtin command or
  an external process (`resolve_external_command`).
- `state/` — runtime state: `scope.rs` (lexical scopes/namespaces, `Scope::create_root()`
  is the global namespace all builtins are declared into), `contexts.rs` (`CommandContext`,
  the struct every command function receives: input/output streams, arguments, scope,
  global state), `global_state.rs`, `handles.rs`/`id.rs` (job/thread handles).
- `value/` — the dynamic value system: `Value` and `ValueType` (Crush's runtime type tags).
- `data/` — concrete value representations: `table.rs` (the core row/column data type
  pipes carry), `struct.rs`, `dict.rs`, `list.rs`, `binary.rs`.
- `pipe.rs` — the typed pipe/stream primitives connecting commands (streams of `Row`s,
  not bytes).
- `signature/` — argument *types* usable in `#[signature]` structs beyond plain Rust
  primitives (`Files`, `Patterns`, `BinaryInput`, `Number`, `Text`, ...).
- `serialization/` — the "pup" binary serialization format used to pass values between
  Crush processes (see `--pup` mode in `main.rs`, used e.g. by `sudo` and `remote:exec`
  to run closures in a child process).
- `completion/` — tab completion.
- `interactive/` — the rustyline-based REPL (`rustyline_helper.rs` wires completion/
  highlighting into rustyline).
- `errors.rs` — `CrushResult<T>` / `CrushError` are used everywhere instead of a generic
  error type; helper constructors like `command_error()`, `data_error()`,
  `argument_error()`, `compile_error()` build errors with the right `CrushErrorType`.

### `src/builtins` — builtin commands

Each module groups related commands and exposes a `declare(root: &Scope) -> CrushResult<()>`
function that registers its commands into the namespace; `src/builtins/mod.rs::declare()`
calls every module's `declare()` in turn to populate the root scope at startup. Submodules
mirror this pattern recursively (e.g. `builtins/fs/mod.rs`, `builtins/stream/mod.rs`,
`builtins/types/mod.rs`).

`types/` holds the builtin methods on Crush's own value types (`string`, `integer`, `list`,
`dict`, `time`, `table`, `table_input_stream`/`table_output_stream`, `re`, `glob`, ...).
`stream/` holds the SQL-like pipeline commands (`where`, `sort`, `group`, `join`, `zip`,
`aggregation`, `uniq`, ...) that operate on table streams. `control/` holds language
control-flow builtins (`if`, `for`, `while`, `loop`, `cmd`, `help`, `schedule`, `timeit`).
Some modules are gated with `#[cfg(target_os = "linux")]` (`dbus`, `systemd`) since they
wrap Linux-only system APIs.

### Writing a new builtin command

This is the standard, recurring pattern (see any file under `src/builtins/` for real
examples, e.g. `src/builtins/fs/files.rs`):

```rust
#[signature(
    module.command_name,
    can_block = true,                  // whether it may block / should run on a worker thread
    short = "One-line description",
    output = Known(ValueType::...),    // or Unknown
)]
struct CommandName {
    #[unnamed()]
    #[description("...")]
    positional_arg: SomeType,
    #[description("...")]
    #[default(true)]
    named_flag: bool,
}

fn command_name(context: CommandContext) -> CrushResult<()> {
    let cfg: CommandName = CommandName::parse(context.arguments, &context.global_state.printer())?;
    // ... use cfg, read from context.input, write to context.output ...
    context.output.send(...)
}
```

The `#[signature(...)]` macro (from the `signature` crate) generates the parser, help
text, and tab-completion metadata from the struct's fields/attributes. The command is
wired up by calling `CommandName::declare(env)?` from the enclosing module's `declare()`.

## Documentation

`docs/` contains user-facing docs: `overview.md` (a short narrative tour),
`language_reference.md` (syntax and core language features — expression mode, pattern
matching, error handling, background jobs, warnings, etc. — in depth), and `config.md`
(configuration) are hand-written.

`docs/builtins.html` is different: a self-contained, searchable HTML reference page
covering every builtin command and namespace (grouped by namespace, each with its own
description and index, plus a search box with autocomplete), auto-generated by
introspecting the running binary's own root scope. It does not update itself —
regenerate it after any change that adds, removes, or redocuments a builtin:
`cargo build && ./target/debug/crush generate_docs.crush > docs/builtins.html`.

GitHub shows `docs/builtins.html` as raw source rather than rendering it, so it's also
published via GitHub Pages (Settings → Pages → deploy from `master`/`docs`) at
https://liljencrantz.github.io/crush/builtins.html — that's the link to give a human;
regenerating and committing the file is enough to update it, no separate deploy step.
