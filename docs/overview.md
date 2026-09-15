# Crush overview

Crush is an attempt to make a traditional command line shell that is also a modern
programming language. It has the features one would expect from a modern programming
language -- a type system, closures, lexical scoping -- with a syntax geared toward both
batch and interactive shell usage.

For the full syntax and language semantics, see the
[language reference](language_reference.md); this document is a narrative tour of what
makes Crush different.

## What features of a traditional shell does Crush retain?

The basic structure of the Crush language resembles a regular shell like bash. How you
invoke commands, pass arguments, and set up pipelines are unchanged, as is the central
concept of a current working directory. Trivial invocations like `ls` or `find .. |
count` look the same -- but under the hood they're quite different, and nearly
everything beyond that is different too.

## Scratching the surface

Let's start with two trivial commands: listing files in the current directory, and
counting them.

```shell script
crush# files
user size  modified                  type      file
fox  2_279 2020-03-07 13:00:33 +0100 file      ideas
fox  4_096 2019-11-22 21:56:30 +0100 directory target
...

crush# files | count
14
```

This all looks familiar, but appearances are deceiving. `files` is a Crush builtin, and
its output isn't sent over a Unix pipe as bytes -- it's sent over a Crush pipe as a
*table of rows*. Crush provides SQL-like commands to sort, filter, aggregate, and group
that data:

```shell script
# Sort the table by the size column
crush# files | sort size
user size modified                  type      file
fox    31 2019-10-03 13:43:12 +0200 file      .gitignore
fox    75 2020-03-07 17:09:15 +0100 file      build.rs
...

# Filter to only show directories
crush# files | where {($type == directory)}
user size  modified                  type      file
fox  4_096 2019-11-22 21:56:30 +0100 directory target
fox  4_096 2020-03-16 14:11:39 +0100 directory .idea
```

Because Crush's output is a stream of rows with columns, sorting by an arbitrary column
or filtering on an arbitrary logical expression is easy -- and because the components
that do this are generic and reusable, the same tools work on data from any source: a
process list, a JSON file, an HTTP response, and so on.

## Reading and writing files

Traditional shell I/O is a stream of bytes. Since Crush streams are typed, Crush instead
has command pairs for serializing and deserializing file formats -- e.g. `json:from`/
`json:to` for JSON, `toml:from`/`toml:to` for TOML:

| Namespace | Description                                       |
|-----------|----------------------------------------------------|
| `bin`     | Binary stream, i.e. no encoding at all.             |
| `csv`     | Comma separated values. Only decoding supported.    |
| `json`    | JSON file format.                                   |
| `lines`   | Lines of text files.                                |
| `pup`     | The native file format of Crush.                    |
| `toml`    | TOML file format.                                   |
| `yaml`    | YAML file format.                                   |

```shell script
# Dump the output of files to listing.json
crush# files | json:to ./listing.json

# Read Cargo.toml and extract its dependencies field
crush# toml:from Cargo.toml | member dependencies
```

If a deserializer isn't given an input file, it reads from its input instead (which must
be `binary`/`binary_stream`); if a serializer isn't given an output file, it writes a
binary stream to its output instead of a file:

```shell script
crush# list:of "carrot" "carrot" "acorn" | json:to
[
  "carrot",
  "carrot",
  "acorn"
]
```

`pup`, Crush's own native format, is the only one of these that can losslessly represent
every Crush value, including closures and class instances -- at the cost of being
useless for sharing data with anything that isn't Crush.

This one-namespace-per-format shape -- rather than a single `serialize`/`deserialize`
command taking a `format=json` argument -- is a deliberate, recurring choice in Crush's
own builtins: prefer several small, focused, composable commands over one command
configured by a flag. Each format namespace only needs to know its own format, and
supporting a new one means adding a namespace, not touching a shared command that
already handles every other one.

## Expression mode and pattern matching

For math and comparisons, enter *expression mode* with parentheses:

```shell script
crush# (5 + 6)
11
crush# (4 > 5)
false
```

For matching text against a pattern, Crush has globs (shell-style wildcards), regular
expressions, and a `match` block for branching on a value's shape. See
[Expression mode](language_reference.md#expression-mode) and
[Pattern matching](language_reference.md#pattern-matching) in the language reference for
the details.

## Variables, of any type

Use the `$` sigil for a variable, and `:=` to declare it before first use:

```shell script
crush# $some_number := 4
crush# $some_number * 5
20
crush# $some_number = 6      # := declares, = reassigns
```

Variables can hold a value of any type in the type system -- lists, dicts, structs,
closures, tables, and more -- with no implicit conversion between types. See
[The type system](language_reference.md#the-type-system) for the full list.

## Blocks and closures

Braces (`{}`) create a block of code, which can be assigned to a variable to define your
own command:

```shell script
crush# $greet := {echo "Hello!"}
crush# greet
Hello!
```

A block with a declared parameter list is a closure, with optional typed and/or
defaulted parameters. See [Blocks and closures](language_reference.md#blocks-and-closures)
for closures, early return, and the `@`/`@@` spread operators.

## Namespaces and methods

Crush relies on namespaces to organize commands and avoid clashes, and uses `:` (not
`.`, which is too common in file names) for both namespace and member access:

```shell script
crush# help $sort
stream:sort [field=string...] [--reverse] [--case_insensitive]

    Sort input stream based on one or more of it's columns
    ...
```

## Streams: lazy and materialized

Most command output is a stream that can only be traversed once -- reading it twice
produces nothing the second time. This lets Crush work on data sets larger than memory
and run pipeline stages concurrently, but it also means a stream you want to read more
than once needs to be turned into a reusable form first, with `materialize`. See
[Streams](language_reference.md#streams) for the full explanation and examples.

## Calling external commands

If no builtin of a given name exists, Crush looks for and runs an external command. This
part of Crush is still a proof of concept -- no tty is handed over or emulated, so
interactive terminal programs don't work -- but most non-interactive commands work as
expected, with some conveniences for translating named arguments into CLI flags. See
[Calling external commands](language_reference.md#calling-external-commands) for
details.

## Executing commands remotely or as other users

Traditional shells run commands elsewhere (via `ssh`, `sudo`, ...) by expanding the
command and its arguments locally and transferring the result as text -- which leads to
a multitude of issues with double expansion, whitespace splitting, and escaping.

Crush instead passes a *closure* as an argument to the command. The command serializes
the closure, transfers it to the remote process, runs it there, and serializes the
result back:

```shell script
# Run a closure as another user
crush# users[root]:do {./carrot:chown group="rabbit"}

# Run a closure on a remote host
crush# remote:exec {uptime} "popplar.meadow"

# Run a closure on several remote hosts at once
crush# remote:pexec {uptime} "popplar.meadow" "elm.meadow"
```

## Creating custom types

Use `struct:of` for a simple, immutable, ad-hoc value, or `class` for a real type with
methods and (optional) inheritance:

```shell script
$Point := $(class)
$Point:__init__ = {|$x:$float $y:$float| $this:x = $x; $this:y = $y}
$Point:len = {|| math:sqrt (($this:x * $this:x) + ($this:y * $this:y))}

$p := $(Point:new x=1.0 y=2.0)
$p:len
```

See [Creating custom types](language_reference.md#creating-custom-types) for the full
example, including operator overloading.

## Summary

Hopefully that's enough to give a sense of what problems Crush is trying to solve, and
whether the project is of interest to you. For everything not covered here -- error
handling, background jobs, the warning log, the full operator table, and more -- see the
[language reference](language_reference.md).
