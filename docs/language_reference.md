# Crush language reference

This document describes Crush's syntax and core language features in depth. If you're
new to Crush, read [the overview](overview.md) first for a narrative tour; come back here
for the details it doesn't have room for.

## Commands

The structure of a Crush command is a space separated list. The first element of the
list is the command, the remaining elements are the arguments:

```shell script
echo 5
git commit message="This commit is amazing"
```

### Named and unnamed arguments

Arguments to commands can be passed named or unnamed, and it's often possible to use
one, the other, or a combination of both. The following three invocations are
equivalent:

```shell script
http uri="http://example.com" method=get
http "http://example.com" get
http "http://example.com" method=get
```

Argument mapping works as follows:

* First, all named arguments are assigned.
* Then, each unnamed argument is assigned to the first parameter that doesn't already
  have a value.

A command can also declare that stray named or stray unnamed arguments (the ones left
over after normal assignment) should be collected into a dict or list instead of being
rejected -- see [`@`/`@@`](#the--and--operators) below.

It's common to want to pass a boolean argument, so Crush has a shorthand for it: one or
two leading dashes, like `--foo` or `-foo`, is equivalent to `foo=$true`.

## Jobs and pipelines

Commands accept a single value as their input and produce a single value as their
output, in addition to their arguments. The input and output of a command are connected
via a pipeline:

```shell script
host:procs | sort cpu
```

Many commands consume and produce table streams as input and output. These commands run
concurrently, so the whole result need not be produced before the next step in the
pipeline begins work (see [Streams](#streams) below).

The separation of concerns between arguments and input/output is that arguments
configure *how* data should be processed, while the input is the data to process and
the output is where the processed data ends up.

## Literals

A character sequence enclosed in double quotes is a string literal, e.g. `"hello"`.
Unquoted character sequences containing only letters, digits and underscore are also
strings, e.g. `user` or `hat`.

An unquoted character sequence containing a wildcard character (`*` or `?`) is a
**glob**, an object used for pattern-matching against text -- see
[Pattern matching](#pattern-matching) below.

A character sequence enclosed in single quotes is a **file** literal, e.g.
`'Cargo.toml'`. An unquoted sequence that contains a dot (`.`) or a slash (`/`), or that
begins with a tilde (`~`), is also interpreted as a file literal, e.g. `Cargo.toml` or
`~/.ssh`. This matters beyond just picking a type: a command argument that looks like a
file (because it contains a dot) is a `file` value, not a `string`, even if the command
expects a string -- e.g. `like foo.txt *.txt` fails because `foo.txt` parses as a file,
and `like`'s argument must be a string; `like "foo.txt" *.txt` (explicitly quoted) works.

A character sequence starting with a dollar sign (`$`) is a variable lookup. The first
word of a command (i.e. the command name) is interpreted as a variable lookup even
without the leading `$` -- commands live in the same namespace as all other variables,
which is why `$echo` and `echo` refer to the same value.

## Expression mode

Crush has a second syntax mode, entered with parentheses, for writing mathematical and
logical expressions with conventional infix operators and precedence:

```shell script
crush# (5 + 6)
11
crush# (1 + 2 * 3)
7
```

Comparisons use `>`, `<`, `<=`, `>=`, `==` and `!=`; comparing values of different types
is an error:

```shell script
crush# (4 > 5)
false
crush# (40.0 > 5)
Error: Values of type float and integer can't be compared with each other
```

Expression mode is a fully functional secondary syntax, not just a calculator: pipes
work, and so does calling commands -- but a call's arguments go inside parentheses after
the command name, unlike in command mode's space-separated form:

```shell script
# An entire pipeline written in expression mode
(files() | sort("file", reverse=true) | where({size < 1000}))

# A method call's argument needs its own parens too
("Hello, {}":format($name))
```

`and`/`or` work as infix operators in expression mode; in command mode they're ordinary
commands taking two values (`or $false $true`):

```shell script
crush# ($false or $true)
true
```

A bare expression at the very start of a script statement (with nothing enclosing it)
doesn't parse reliably today -- wrap it in parentheses, even when nesting it as another
command's argument, e.g. `assert (1 == 1)` rather than `assert 1 == 1`.

## Operators

Crush provides operators for arithmetic, comparison, and a few other things that read
better as symbols than as commands. Grouped roughly by precedence, highest first:

| Operator                    | Example              | Description                                                     |
|------------------------------|-----------------------|-------------------------------------------------------------------|
| `:=` `=`                    | `$foo := 7`           | Declare a new variable, or reassign an existing one                |
| `and` `or`                  | `$a and $b`           | Logical operators. Also work as ordinary commands: `or $a $b`      |
| `==` `!=` `>` `>=` `<` `<=` | `$foo > 5`            | Compare two values                                                 |
| `=~`                        | `abbbbbc =~ ^(ab+c)`  | True if the left value matches the right-hand pattern              |
| `+` `-`                     | `1 + 1`, `-5`         | Addition, subtraction, and unary negation                          |
| `*` `/`                     | `5 * 5`, `7 / 2`      | Multiplication and division (truncating for two integers)          |
| `typeof`                    | `typeof $foo`         | The type of a value                                                |
| `not`                       | `not $true`           | Logical negation. Also works as an ordinary command                |
| `@` `@@`                    | see below             | Argument/parameter list spreading                                  |

There's no modulo/remainder *operator* -- use the `mod` (least positive residue) or
`rem` (ordinary remainder) methods on a number instead, e.g. `7:mod 2`.

`=~` currently has no negated form -- there is no working `!~` yet. Use `not (... =~ ...)`
instead. A glob *literal* (e.g. `*.txt`) doesn't parse directly on the right of `=~`
inside expression mode; assign it to a variable first (`$g := *.txt`) and match against
that, or use `like`/`not_like` in command mode instead (see
[Pattern matching](#pattern-matching)).

### The `@` and `@@` operators

`@` spreads a list as a sequence of unnamed arguments, and `@@` spreads a dict as a
sequence of named arguments -- both at call sites and in a closure's own parameter list,
where they instead *collect* stray arguments:

```shell script
# @args collects every unnamed argument into a list; @@kwargs collects every named
# argument not otherwise bound in the parameter list into a dict.
$print_everything := {|@ $unnamed @@ $named| echo "Named" $named "Unnamed" $unnamed}
$print_everything 1 2 x=3 y=4

# The mirrored use, at a call site: spread a list/dict back out into arguments. This
# defines an `ls` that forwards whatever it's given to `files`, then selects one column.
$ls := {|@ $args @@ $kwargs| files @ $args @@ $kwargs | select file}
```

## Pattern matching

Crush has three ways to match a value against a pattern, depending on how much power
you need.

**Globs** support simple wildcards -- `*` for any run of characters, `?` for a single
character, `**` to recurse into subdirectories -- and are the type most shell users
already know from filename expansion:

```shell script
crush# files *.md
crush# files ????????
# Count the lines of rust code in this tree
crush# lines:from **.rs | count
```

A glob is a value in its own right, not automatically expanded -- it's passed to
whatever command receives it, which chooses what to match it against. The `like`/
`not_like` methods match an explicit string against a glob:

```shell script
crush# like "foo.txt" *.txt
true
crush# not_like "foo.txt" *.md
true
```

Note that these do an *exact* match against the pattern (respecting the wildcards) --
they're not a substring search.

**Regular expressions** support the usual regex syntax and are constructed with
`^(...)`:

```shell script
crush# like abbbbbc ^(ab+c)
true
crush# ^(a+):replace baalaa a
balaa
crush# ^(a+):replace_all baalaa a
bala
```

The `=~` operator (see [Operators](#operators) above) matches a value against a
glob or regex the same way `like` does, but reads naturally in expression mode:

```shell script
crush# (abbbbbc =~ ^(ab+c))
true
```

**`match`** branches on a value against a sequence of typed cases -- useful when you'd
otherwise write a chain of `if`/`else if`:

```shell script
match $x {
    case 2 {echo "$x is 2"}
    any $(seq 5 10) {echo "$x is between 5 and 10"}
    is $string {echo "$x is a string"}
    default {echo "I don't know what $x is"}
}
```

Each arm is tried in order; the first that matches runs and the rest are skipped:

* `case <value> {...}` matches if the subject equals `<value>`.
* `any <stream> {...}` matches if the subject equals any value produced by `<stream>`
  (e.g. a list or `$(seq 5 10)`).
* `is <type> {...}` matches if the subject's type is `<type>`.
* `default {...}` always matches.

If nothing matches and there's no `default` arm, `match` fails with an error.

## Command substitutions

To use the output of one command as an *argument* to another (rather than as its input),
put the command inside dollar-parentheses (`$()`):

```shell script
"Hello, {name}":format name=$(users:me:name)
```

If the substituted command produces a stream, the outer command runs concurrently with
it and may finish first. This example creates a table stream and assigns it to a
variable without consuming it -- the `files` command blocks once its output buffer fills,
since nothing is reading from it yet:

```shell script
$all_the_files := $(files --recurse /)
$all_the_files | head 1
```

## Namespaces, members and methods

Crush relies heavily on namespaces to separate commands and avoid name clashes. Members
(namespace contents, struct/dict fields, and value methods) are all accessed with the
`:` operator -- other languages tend to use `.`, but that's a very common character in
file names, so Crush needed something else.

A few of the built-in namespaces:

* `crush`, runtime information about the current shell and how to reconfigure it,
* `users`, information about the users of this system,
* `fs`, the file system (`fs:files`, `fs:cwd`, `fs:watch`, ...),
* `host`, information about the current host: hostname, CPU, memory, OS.

Most types have useful methods too. Files, for instance, have `exists`:

```shell script
crush# .:exists
true
```

Filesystem metadata itself comes from the `fs` namespace rather than a file method,
since it takes a real syscall to answer:

```shell script
crush# fs:stat . | select is_dir file
is_dir file
true   .
```

## The type system

Crush values are typed. Most commands operate on streams of tabular data, where each
cell can be any of these types:

* `list`, a mutable list of items, usually of one type,
* `dict`, a mutable mapping between a pair of types (not every type can be a key),
* `string`, `glob`, `re`, `file`, `binary` -- see `help $string` for how these five
  "sequence of stuff" types relate to and differ from each other,
* `bool`, `integer`, `float`,
* `struct`, a mapping from name to value with a fixed set of fields,
* `table`, essentially a list where every element is a struct with the same fields,
* `table_input_stream`/`table_output_stream`, like a table but can only be traversed
  once (see [Streams](#streams) below),
* `binary_stream`, like `binary` but can only be traversed once,
* `type`, and
* `command`, either a closure or a builtin.

`help $<type>` (e.g. `help $list`, `help $time`) documents each type's own creation syntax,
mutability, and related types in more depth than fits here.

### Creating custom types

Use `struct:of` for a simple, immutable, ad-hoc mapping of names to values:

```shell script
crush# $p := $(struct:of x=1 y=2)
crush# $p:x
1
```

Use `class` to define a real type with methods and (optionally) inheritance:

```shell script
$Point := $(class)

$Point:__init__ = {
    |$x:$float $y:$float|
    $this:x = $x
    $this:y = $y
}

$Point:len = {
    ||
    math:sqrt (($this:x * $this:x) + ($this:y * $this:y))
}

$Point:__add__ = {
    |$other|
    Point:new x=($this:x + $other:x) y=($this:y + $other:y)
}

$p := $(Point:new x=1.0 y=2.0)
$p:len
```

`class` creates a struct with a `new` method; calling `new` creates an instance and
calls `__init__` (if defined), passing along any arguments. Add methods by assigning to
the class; add instance fields by assigning to `$this` inside `__init__`. Pass a parent
class to `class` for single inheritance.

## Blocks and closures

Braces (`{}`) create a block of code. Named arguments passed at invocation are added to
the block's local scope:

```shell script
crush# $print_a := {echo $a}
crush# print_a a="Greetings"
Greetings
```

A block with a declared parameter list is a **closure**, which adds type safety and
named positional parameters:

```shell script
crush# $add := {|$a $b| $a + $b}
crush# $add 1 2
3
```

Closures can return early with the `return` command, which unwinds the entire closure
(not just the innermost block):

```shell script
{
    ||
    if $(check_early_exit) {
        return
    }
    ...
}
```

A parameter can declare a type and/or a default value:

```shell script
# $b must be an integer, defaulting to 7 if not given
$f := {|$a $b: $integer = 7| echo $a $b}
$f 1
```

A parameter with a default can only be overridden by naming it (`$f 1 b=2`) -- a second
*positional* argument does not fill it in.

`@`/`@@` also work in a closure's own parameter list, to collect stray arguments -- see
[The `@` and `@@` operators](#the--and--operators) above.

## Error handling

By default, a command that fails aborts the rest of the script -- there's no implicit
"print an error and keep going." A script that runs several independent steps and wants
to survive one failing needs to handle that explicitly.

**`try`/`catch`** runs a block, recovering from any error it produces:

```shell script
try {
  risky:command
} catch {
  |$error| echo ("Recovered: {}":format($error))
}
```

If `body` fails, execution of `body` stops at the failing statement and `catch` (if
given) runs instead, receiving the error message as a plain string. Either way, the
error does not propagate past `try` -- with no `catch` at all, `try` just recovers
silently, equivalent to an empty `catch`.

**`assert`** is the simplest way to raise an error deliberately, e.g. inside a script or
a closure's own validation:

```shell script
crush# assert $false "custom failure message"
Error: custom failure message
```

Unlike some languages, there's no separate "exception object" hierarchy to catch by
type -- `catch`'s argument is always just the error's message as a string.

## Background jobs

A job started with a trailing `&` runs in the background: control returns to the script
immediately, and the job's eventual result is registered for later retrieval instead of
being waited for.

```shell script
$job_id := $(sleep $(duration:of seconds=2) &)
# ... do other work while it runs ...
fg $job_id
```

`fg` waits for and returns a backgrounded job's result. `crush:jobs` lists every
currently running job (including nested ones, like command substitutions running as
part of a larger job), with `id`/`parent`/`description`/`type`/`status` columns.
`crush:pause`/`bg` pause a running job and later resume it in the background (as opposed
to `&`, which starts a job in the background from the moment it's created).

For two jobs that need to communicate directly -- one producing data the other consumes,
both running concurrently -- create a **pipe** by calling `:pipe` on a
`table_input_stream` type:

```shell script
# Create a pipe
$pipe := $($(table_input_stream value=$integer):pipe)
# Create a job that writes integers 1 through 999 to the pipe, backgrounded
$write_job_handle := $(seq 1 1000 | pipe:write &)
# Create a second job that reads from the pipe and sums the integers, backgrounded
$sum_job_handle := $(pipe:read | sum &)
# Put the writer job in the foreground so it fully finishes before the pipe is closed
fg $write_job_handle
# Close the pipe so the reader can finish
pipe:close
# Put the sum job in the foreground to get its result
fg $sum_job_handle
```

Always `fg` every writer job before calling `pipe:close` -- a writer's own worker thread
isn't synchronized with `close`, so closing the pipe before a writer has actually run can
silently drop that writer's contribution.

## Warnings

Some commands process many independent items (rows, hosts, files) and are designed to
report a per-item failure rather than aborting the whole command over one bad item.
Rather than just printing and moving on, they record the failure in a bounded warning
log:

```shell script
crush# list:of 1 2 3 | each {assert ($value != 2) "value was 2"}
crush# crush:warnings
timestamp                 command message     file  location
2024-01-01 00:00:00 +0000 <block> value was 2 ...    ...
```

`each`, `where`, `group`, `files`, a class's `__init__`, and `remote:pexec` (a failed
host produces no row, not an error) all report into this log instead of failing
outright. `crush:warnings` lists the most recent entries; `crush:warning_limit:get`/
`:set` control how many are kept (100 by default) before the oldest is evicted. In
interactive mode, a warning is also printed immediately as it happens; in a script, the
only way to notice one happened is to check `crush:warnings` (or, for commands that
report per-item this way, to compare how much output you got against how much you
expected -- see e.g. `remote:pexec`'s own documentation).

## Streams

### Semi-lazy stream evaluation

Assigning the output of a streaming command to a variable stores a `table_input_stream`,
not the data itself:

```shell script
crush# $all_the_files := $(files --recurse /)
```

Control returns immediately -- `files` only produces output as its stream buffer is
consumed. Reading the variable (`$all_the_files`) drains the whole stream at once;
piping it through `head 1` consumes exactly one row, and can be re-run until the stream
is empty.

### Materialized data

A `table_input_stream` (or `binary_stream`) can only be traversed once -- reading it a
second time produces nothing. This is often what you want: it lets a pipeline work on
data sets larger than memory, and lets different stages of a pipeline run concurrently.
But sometimes you want to read the same data more than once. `materialize` converts a
value's transient (stream) components into their reusable equivalents (`table_input_stream`
into `table`, `binary_stream` into `binary`), recursively:

```shell script
crush# $f := $(files)
crush# $f
... (rows printed) ...
crush# $f
crush#                      # nothing -- already consumed

crush# $m := $(files | materialize)
crush# $m
... (rows printed) ...
crush# $m
... (same rows printed again) ...
```

## Calling external commands

If no internal command of a given name exists, Crush looks for an external command and
runs it if found. Crush doesn't hand over the tty or emulate one, so interactive
terminal programs and heavily escape-sequence-dependent output don't work -- this part
of Crush is best considered a proof of concept, though most non-interactive commands
work as expected.

Two shortcuts make external commands nicer to call:

* Named arguments become options: a single-character name becomes a single-hyphen
  option, a multi-character name becomes a GNU-style double-hyphen long option -- e.g.
  `git commit m="hello"` becomes `git commit -m "hello"`, and
  `git commit message="hello"` becomes `git commit --message "hello"`.
* A named argument with value `$true` becomes a flag with no value -- `git commit
  a=$true` becomes `git commit -a`.
