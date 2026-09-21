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
output, in addition to their arguments. Commands can be chained together into a 
pipeline by using the `|` pipeline operator. The output of one command becomes the input
for the next command in a pipeline:

```shell script
# Sort list of processes by cumulative CPU usage. Only print the top ten.
host:procs | sort cpu | head
```

Many commands consume and produce table streams as input and output. Table streams are
transmitted one row at a time, meaning that as soon as the first row of output is 
produced by one command, the next command can begin processing it. This means that
compute constrained pipelines are processed concurrently by at least as many CPU 
cores as there are pipeline steps.

The separation of concerns between arguments and input/output is that arguments
are meant to configure *how* data is processed, while the input is the data to 
process and the output is where the processed data ends up.

## Streams

### Processing streams

The point of having almost everything in Crush be a table stream is that crush can
provide you with a set of tools to manipulate these streams. These tools work the same
on any type of stream. 

Some stream commands are described below. Stream manipulation commands in crush 
live in the `$stream` namespace. To list them, start crush and write `help $stream`.
For help on an individual stream command, like `seq`, write `help $seq`.

#### `head`

Passes through a set number of rows from the start of the stream and truncates the rest.

```
# Show the first ten lines of README.md
lines:from README.md | head
```

#### `tail`

Passes through a set number of rows from the end of the stream and skips the rest.

```
# Show the last ten lines of README.md
lines:from README.md | tail
```

#### `sort`

Sorts the stream on the specified column. 

```
# Sort the files of the current working directory by size
files | sort size
```

#### `where`

Only passes through rows where a given condition holds true.

```
# Show all subdirectories of current directory
files | where {eq $type directory}
```

The `eq` command tests for equality, there are other comparison commands, check `help $comp` for the full list.

#### `select`

Passes on some columns unchanged, and can add new ones computed from the others.

```
# Show only the file names, discarding every other column
files | select file
```

#### `group`

Groups rows that share the same value in one or more columns into a single output row
per group, aggregating the rest of each group's rows with the given command(s).

```
# Count how many files and how many directories are in the current directory
files | group type count={count}
```

#### `uniq`

Passes through only the first row for each distinct value of the specified column,
dropping every later row that repeats it.

```
# List one file of each type (file, directory, ...) found in the current directory
files | uniq type
```

#### `join`

Joins two streams together on a shared key column, producing one output row for every
matching pair.

```
# Join two small tables together on their shared id column
$fruit := $(csv:from "1,apple\n2,pear\n" id=$integer name=$string)
$stock := $(csv:from "1,5\n2,3\n" id=$integer count=$integer)
join id=$fruit id=$stock
```

#### Aggregation commands

Some commands operate on a stream and instead of producing a new stream, aggregate that
stream into a single value.

Commands like `count`, `sum`, `median`, `avg`, `min`, and `max` operate on a column of a stream 
and return a single value:

```shell
# Calculate the number of files in the current directory
files | count

# Calculate the file size of all files in the current directory
files | sum size

# Find the CPU usage of the process that has used the most CPU
host:procs | max cpu

```

Another type of aggregation are the `list:collect` and `dict:collect` commands, that
collect all the values of a stream into either a list or a dict:

```shell
# Collect all the names of all the files in the current directory into a list
files | list:collect

# Create a dict mapping from process id to command name for all currently running processes
host:procs | dict:collect pid name
```

Aggregation commands are often used together with the `group` command. When you create a new
column using a command in `group`, you often use this to aggregate values:

```shell
# Calculate accumulated CPU usage per-user
host:procs | group user total_cpu_usage={sum cpu}

# Calculate median memory usage (resident set size) per-user
host:procs |group user total_cpu_usage={median rss}
```

### Lazy stream evaluation

Assigning the output of a streaming command to a variable stores a `table_input_stream`,
not the data itself:

```shell script
crush# $all_the_files := $(files --recurse /)
```

The command finishes and control returns to the shell immediately. The `files` command 
will begin writing rows to its output buffer in the background, but because the buffer 
is bounded it will start blocking once it is full. If you read the value of the variable 
(for example by simply typing `$all_the_files`), you will drain the whole stream to the 
screen which will take a very long time. If you instead pipe it through 
`head 1` (i.e. `$all_the_files | head 1`), you will consume exactly one row. This command can
be repeated over and over  until the stream is empty, and every time you do so one new
line will be returned.

### Materialized data

A `table_input_stream` (or `binary_stream`) can only be traversed once - consuming it a
second time produces an empty stream. This is often what you want: it lets a pipeline work on
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

A whole number immediately followed (no space) by one of `ns`, `ms`, `m`, `h`, or `s` is
a **duration** literal -- nanoseconds, milliseconds, minutes, hours, or seconds,
respectively -- e.g. `5s` is exactly `duration:of seconds=5`. There's no literal syntax
for a duration made of more than one unit (e.g. an hour and a half); use `+` on two
duration values instead, e.g. `1h + 30m`.

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

## Flow control (`if`/`else`, `while`, `for`, `try`/`catch`, `and`, `or` and `match`)

Crush has several built-in commands that take one or more blocks of code (written as
`{...}`) and decide whether, how many times, or under what conditions to run them. Like
any other command, they all work in both command mode and expression mode -- the
examples below are written in command mode; see [Expression
mode](#expression-mode) for the (mostly cosmetic) differences.

### `if`/`else`

`if` takes a boolean condition and a block to run when it's true, with an optional
`else` clause -- the literal word `else`, not just any second block -- for when it's
false:

```shell script
$a := 15
if ($a > 10) {
    echo big
} else {
    echo small
}
```

With no `else` clause, a false condition simply makes `if` does nothing.

### `while`

`while` takes a condition block and a body block. Unlike `if`'s condition, `while`'s
condition is itself a block (not a plain boolean), because it's re-evaluated before
every lap:

```shell script
$i := 0
while {lt $i 5} {
    echo $i
    $i = ($i + 1)
}
```

The body is optional. Without one, the condition block is both the loop's test *and*
its work -- it keeps running for as long as it returns `$true`, so the loop's exit check
effectively happens at the end of each lap instead of the start:

```shell script
$i := 0
while {
    $i = ($i + 1)
    echo $("lap {}":format $i)
    ($i <= 3)
}
```

### `for`

`for` runs a block once per row of an input stream, binding each row to a name given as
`name=stream`:

```shell script
for i=$(seq 1 5) {
    echo $i
}
```

When the stream has more than one column, each row is bound as a struct instead of a
bare value, so its columns are reachable by name:

```shell script
for row=$(csv:from "1,apple\n2,pear\n" id=$integer name=$string) {
    echo ("{}: {}":format($row:id, $row:name))
}
```

### `loop`

`loop` repeats its body forever, until stopped with `break`:

```shell script
$n := 0
loop {
    if ($n >= 3) {
        break
    }
    echo ("lap {}":format($n))
    $n = ($n + 1)
}
```

### `break` and `continue`

`break` stops the nearest enclosing loop (`while`, `for`, or `loop`) immediately;
`continue` skips the rest of the current lap and moves straight to the next one. Both
look outward through any nested `if`, `try`, or other non-loop block to find that
enclosing loop, so they work from arbitrarily deep inside one -- and calling either one
outside of any loop at all is an error:

```shell script
for i=$(seq 1 10) {
    if ($i:mod(2) == 0) {
        continue
    }
    if ($i > 7) {
        break
    }
    echo $i
}
```

### `and`/`or`

`and` and `or` combine several conditions, short-circuiting as soon as the result is
known -- `and` stops at the first `$false`, `or` stops at the first `$true`, and neither
evaluates anything after that. Each argument can be a plain boolean or a block that
produces one, mixed freely:

```shell script
assert ($true and {1 == 1})    # every condition true -> true
assert ($false or {1 == 1})    # at least one condition true -> true
```

In expression mode, `and`/`or` are also available as infix operators of the same name:
`$a and $b`, `$a or $b`.

### `match`

`match` branches on a value against a sequence of typed arms -- see [Pattern
matching](#pattern-matching) below for the full syntax and semantics. A quick example:

```shell script
match $x {
    case 2 {echo "two"}
    is $string {echo "a string"}
    default {echo "something else"}
}
```

### `try`/`catch`

By default, a command that fails aborts the rest of the script -- there's no implicit
"print an error and keep going." `try`/`catch` runs a block and recovers from any error
it produces:

```shell script
try {
    risky:command
} catch {
    |$error| echo ("Recovered: {}":format($error:message))
}
```

If `body` fails, execution of `body` stops at the failing statement, and `catch` (if
given) runs instead, receiving a struct describing the error as its argument: `message`
(the error text), `type` (the error's category -- see below), and `command` (the failing
command's name, as a string, when known -- empty otherwise). Either way, the error does
not propagate past `try` -- with no `catch` at all, `try` just recovers silently,
equivalent to an empty `catch`.

#### Conditional `catch` clauses

A `catch` can filter which errors it handles, and several can be chained onto one `try`
to handle different errors differently. Each `catch` after the first takes an optional
*filter* -- a string, glob, or regex, anything implementing `__is__` (the same mechanism
`like`/`=~`/`match`'s `is` arm use) -- matched against the error's `type`:

```shell script
try {
    risky:command
} catch ^(Serde.*) {
    |$e| echo ("Serialization error: {}":format($e:message))
} catch Dns* {
    |$e| echo ("DNS error: {}":format($e:message))
} catch {
    |$e| echo ("Something else went wrong: {}":format($e:message))
}
```

Clauses are tried in order; the first whose filter matches (or that has no filter at
all) runs, and the rest are skipped. If no clause's filter matches, the error
propagates past `try` normally, exactly as if none of its clauses could ever have
applied to it.

#### Built-in error types

An error's `type` is normally a fixed name tied to whatever failed internally. These are
the ones you're most likely to see and filter `catch` on, grouped by what usually
triggers them:

**General**

| Type | Meaning |
|---|---|
| `GenericError` | A failure with no more specific category. |
| `InvalidArgument` | A command was called with arguments it can't accept -- also what `assert` raises on failure. |
| `InvalidData` | A value was correctly typed but held a bad or unexpected value. |
| `InvalidJump` | `break`, `continue`, or `return` was used somewhere it doesn't apply. |
| `Terminate` | Raised internally when `crush:terminate` is sent to a job. Avoid filtering `catch` on this -- catching it defeats job control. |

**Parsing and conversion**

| Type | Meaning |
|---|---|
| `ParseError`, `LexicalError` | Crush's own parser or lexer rejected a piece of syntax. |
| `ParseIntError`, `ParseFloatError`, `ParseBoolError` | Converting a string to a number or boolean failed, e.g. via `convert`. |
| `ChronoParseError` | Parsing a date/time string failed. |
| `TryFromIntError`, `CharTryFromError` | A numeric or character conversion didn't fit its target type. |
| `OutOfRangeError` | A duration or similar value fell outside its representable range. |

**I/O and the system**

| Type | Meaning |
|---|---|
| `IOError` | Reading, writing, or otherwise touching the filesystem or a pipe failed. |
| `EOFError` | A stream ended before enough data was available. |
| `Utf8Error`, `FromUtf8Error` | Bytes read from somewhere weren't valid UTF-8. |
| `VarError` | Reading an environment variable failed. |
| `ByteUnitError` | Parsing a byte-size value (e.g. `"5MB"`) failed. |
| `MountpointsError` | Reading the system's mount table failed. |
| `BatteryError` | Reading battery information failed. |
| `NixError` | A POSIX system call failed (permissions, no such process, etc.). |
| `NotifyError` | Setting up or reading from a filesystem watch (`fs:watch`) failed. |
| `ReadlineError` | The interactive line editor reported an error. |

**Serialization**

| Type | Meaning |
|---|---|
| `SerdeJsonError`, `SerdeTomlError`, `SerdeTomlSerError`, `SerdeYamlError` | Decoding or encoding JSON/TOML/YAML failed. |
| `SerializationError` | Crush's own internal (`pup`) serialization format failed. |
| `RegexError` | A regular expression was malformed. |
| `NumFormatError` | Formatting a number failed. |
| `FromHexError` | Decoding hex-encoded data failed. |

**Networking and remote execution**

| Type | Meaning |
|---|---|
| `ReqwestError` | An HTTP request failed. |
| `AddrParseError`, `InvalidUri`, `ToStrError` | Parsing a network address, URI, or header failed. |
| `DnsProtoError`, `DnsClientError`, `ResolveConfParseError` | A DNS lookup or its configuration failed. |
| `SSH2Error` | An SSH operation (`ssh:*`) failed. |
| `LoginsError` | Resolving user login/credential information failed. |
| `Netstat2Error` | Reading network connection/socket tables failed. |
| `GrpcError`, `TonicTransportError`, `ProstDecodeError`, `ProstDescriptorError` | A gRPC call or its message encoding failed. |
| `DbusError`, `Roxmltree` | A D-Bus call, or XML parsing related to one, failed. Linux only. |

**Internal**

| Type | Meaning |
|---|---|
| `SendError`, `RecvError`, `RecvTimeoutError`, `SelectTimeoutError`, `PoisonError` | Crush's own internal plumbing between pipeline stages broke down -- almost always because something downstream (e.g. `head`) stopped reading early, not a real failure. |

When in doubt about which type an error actually has, catch it without a filter and
print `$e:type` -- it's always the authoritative name to match against.

#### Custom errors with `throw`

There's no separate "exception object" hierarchy to catch by type -- there's just the
one struct shape described above. `type` is normally a fixed name tied to whatever
failed internally, but `throw` lets a script raise its own error with a custom
`type` instead, so a script or library can define and catch its own error categories:

```shell script
try {
    throw "NotFound" "no such user"
} catch NotFound {
    |$e| echo ("custom: {}":format($e:message))
}
```

Internally, a thrown error's `type` field holds exactly the string given to `throw` --
`"NotFound"` above -- not a fixed variant name the way every other error in the table
above has; that's what lets a script define an open-ended set of its own error
categories instead of being limited to the built-in ones.

**`assert`** is the simplest way to raise an error deliberately, e.g. inside a script or
a closure's own validation -- it raises `InvalidArgument`:

```shell script
crush# assert $false "custom failure message"
Error: custom failure message
```

`try`/`catch` also works directly in expression mode, with the exact same syntax:

```shell script
crush# ($x := (try { convert($integer, "notanumber") } catch {|$e| -1}))
crush# $x
-1
```

## Crush types

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

Every call needs its `()`, even one that takes no arguments at all -- expression mode is
modeled on C-style languages, where a bare name is a reference to a value, not an
invocation of it:

```shell script
crush# $items := $(list:of 1 2 3)
crush# (($items:len()) > 0)
true
crush# (($items:len) > 0)
Error: The two provided values of types command and integer could not be compared
```

Without `()`, `$items:len` is the method itself (a `command` value) rather than the
result of calling it -- the same distinction
[Assignment takes exactly one value](#assignment-takes-exactly-one-value) shows further
down for `$string:upper`.

### Syntactic sugar in expression mode

In regular command mode, `for`, `while`, `if`/`else`, `try`/`catch` and `match` are simple builtin commands.
If they were to work in the same way in expression mode, the syntax would become clumsy, with many unwanted parenthesis.
Instead, the language has added some syntactic sugar to make these work identically to how they work in command mode:

```
(

for $i=seq(5) {
  echo($i)
}

while condition_test() {
  ...
}

if ($a > 10) { 
  "big"
} else { 
  "small" 
}

try {
    throw("DnsTimeout", "no response")
} catch ^(Serde) {
    |$e| "serde"
} catch $(Dns*) {
    |$e| "dns"
}

match $x {
    case 2 {"two"}
    any $(seq 5 10) {"between 5 and 10"}
    is $string {"a string"}
    default {"something else"}
}

)
```

### Operators in expression mode

In expression mode, Crush provides operators for arithmetic, comparison, and a few other things that read
better as symbols than as commands. Grouped roughly by precedence, highest first:

| Operator                    | Example                                                          | Description                                                     |
|------------------------------|------------------------------------------------------------------|-------------------------------------------------------------------|
| `:=` `=`                    | `$foo := 7`                                                      | Declare a new variable, or reassign an existing one                |
| `and` `or`                  | `$a and $b`                                                      | Logical operators. Also work as ordinary commands: `or $a $b`      |
| `==` `!=` `>` `>=` `<` `<=` | `$foo > 5`                                                       | Compare two values                                                 |
| `=~` `!~`                   | `abbbbbc =~ ^(ab+c)`                                             | True/false if the left value matches the right-hand pattern        |
| `+` `-`                     | `1 + 1`, `-5`                                                    | Addition, subtraction, and unary negation                          |
| `*` `/`                     | `5 * 5`, `7 / 2`                                                 | Multiplication and division (truncating for two integers)          |
| `@` `@@`                    | see the separate section on these operators for more information | Argument/parameter list spreading                                  |

There's no modulo/remainder *operator* -- use the `mod` (least positive residue) or
`rem` (ordinary remainder) methods on a number instead, e.g. `7:mod 2`.

### Globs in expression mode

Glob literals (e.g. `*.txt`) only parse in command mode -- expression mode has no glob
literal syntax at all, so `(x =~ *.txt)` fails to parse. This is because of the clash of the `*`
symbol as both the multiplication operator and a glob wildcard. To use a glob from within
expression mode, wrap it in a command substitution instead: `(x =~ $(*.txt))`. 

## Destructuring assignment

A bracketed list of names on the left of `:=`/`=` splits a list, struct, or dict on the
right into one variable per name, positionally. Struct fields and dict entries are both
backed by an order-preserving map, so "positionally" means declaration order for a
struct and insertion order for a dict -- key order is never involved. The number of
names must exactly match the number of elements, or the assignment errors:

```shell script
crush# [$a $b] := $(list:of 1 2)
crush# $a
1
crush# $point := $(struct:of x=10 y=20)
crush# [$x $y] := $point
crush# $y
20

# = destructures into already-declared variables, exactly like plain = does for one
[$a $b] = $(list:of 3 4)
```

`:=` still requires that none of the names already exist in the local scope, and `=`
still requires that all of them do -- both exactly as for a single-target `:=`/`=`.

## Matching

Matching in Crush is built on one mechanism: any value can act as a *pattern*
by implementing an `__is__` method (and, for negation, `__is_not__`), which takes a
value to test and returns a bool. Strings, globs, regular expressions, and types all
implement it, and so can your own custom types -- see the end of this section. These
methods are named with dunders specifically because they're not meant to be called
directly; the interfaces below all just call them for you.

The **`like`** command checks a value against one or more patterns, returning true as
soon as one matches:

```shell script
crush# like "foo.txt" *.txt
true
crush# like "foo.txt" *.md
false
crush# like abbbbbc ^(ab+c)
true
```

A pattern can be a **glob** (shell-style wildcards `*` for any run of characters,
`?` for a single character, `**` to recurse into subdirectories; the type most shell
users already know from filename expansion), a **regular expression** (usual regex
syntax, constructed with `^(...)`), a plain **string** (exact match -- not a substring
search), or a **type** (checks the value's own type, e.g. `like 5 $integer`). Globs
aren't automatically expanded against the filesystem -- a glob is a value in its own
right, passed to whatever command receives it, which decides what to match it against.

The value being tested against a glob, regex, or string pattern can itself be either a
`string` or a `file` -- `like 'foo.txt' *.txt` works the same as `like "foo.txt" *.txt`.

In expression mode, a single pattern can be checked with the **`=~`**/**`!~`**
operators instead, which read more naturally there:

```shell script
crush# (abbbbbc =~ ^(ab+c))
true
crush# (abbbbbc !~ ^(zzz))
true
```

(`=~ y` and `!~ y` desugar to calling `y`'s own `__is__`/`__is_not__` method, which is
why the pattern goes on the right.)

### Matching against a custom class

Because `like`, `=~`/`!~`, and `match`'s `is` arm all just call `__is__`, any type can
be used as a pattern by implementing it:

```shell script
$Even := $(class)
$Even:__is__ = {|$needle| ($needle:mod(2) == 0)}
$even := $(Even:new)

like 4 $even   # true
like 5 $even   # false
```

## Pattern matching

The `match` command branches a value against a sequence of arms, useful when you'd
otherwise write a chain of `if`/`else if`:

```shell script
match $x {
    case 2 {echo "$x is 2"}
    any $(seq 5 10) {echo "$x is between 5 and 10"}
    is *.txt {echo "$x looks like a text file"}
    is $string {echo "$x is a string"}
    default {echo "I don't know what $x is"}
}
```

Each arm is tried in order; the first that matches runs and the rest are skipped:

* `case <value> {...}` matches if the subject equals `<value>`.
* `any <stream> {...}` matches if the subject equals any value produced by `<stream>`
  (e.g. a list or `$(seq 5 10)`).
* `is <pattern> {...}` matches if `<pattern>` (a type, glob, regex, or anything else
  with an `__is__` method) matches the subject -- the same mechanism `like` uses.
* `default {...}` always matches.

If nothing matches and there's no `default` arm, `match` fails with an error.

`match` also works in expression mode, with the exact same arm syntax as
above:

```shell script
crush# $describe := ({|$n| match $n {
    case 2 {"two"}
    is $string {"a string"}
    default {"something else"}
}})
crush# ($describe(2))
two
```

A match arm's value can be any expression, with no restrictions beyond what command
mode's arms already have.

## Assignment takes exactly one value

`:=` and `=` each take exactly one value on the right-hand side. A single token -- a
literal, a `$variable`, or a bare `$value:member` reference with no arguments of its
own -- is used directly, without being called:

```shell script
crush# $x := "hello"
crush# $y := $x
crush# $upper := $string:upper   # the method itself, as a value -- not called
crush# typeof $upper
command
```

A right-hand side that's itself a job -- anything with its own argument list, like a
`:format` call, or a control-flow construct such as `if`/`else` used as an expression --
is *not* automatically run and reduced to a single result first. Command mode just sees
a flat sequence of tokens, and `:=`/`=` only expect a name and one value, so the extra
tokens are rejected:

```shell script
crush# $x := "{}":format "hi"
Error: Stray arguments
```

Wrap the right-hand side in `$(...)` to run it as its own job and substitute the single
result:

```shell script
crush# $x := $("{}":format "hi")
crush# $x
hi

crush# $score := 95
crush# $level := $(if ($score > 90) {"A"} else {"B"})
crush# $level
A
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

**`var:use`** widens unqualified name resolution to also search a namespace directly,
so its members no longer need the `namespace:` prefix -- **`var:unuse`** reverses it:

```shell script
crush# var:use $math
crush# sqrt 2
1.4142135623730951
crush# var:unuse $math
crush# sqrt 2
Error: Unknown command name `sqrt`
```

`unuse` recursively removes the given scope from the entire parent-scope chain, not
just the current one.

A variable name starting with `__` is reserved for Crush's own internal use.

## Blocks and closures

Braces (`{}`) create a block of code. Named arguments passed at invocation are added to
the block's local scope:

```shell script
crush# $print_a := {echo $a}
crush# print_a a="Greetings"
Greetings
```

The output value of the last command to be executed in a block becomes the output value
of the entire block.

```shell script
files | where {
  $full_user_info := $(users[$user])
  lte $($full_user_info:uid) 500
}
```

### Closures

A block with a list of allowed input parameters at the top is called a **closure**. Closures add several features not 
found in regular blocks:

* parameter validation,
* named positional parameters, 
* collectors for extra named and unnamed arguments, and
* early termination of the block.

#### Closure parameter lists
To make a closure into a block, list the names of the expected parameters between pipes (`|`) at the top of the block:

```shell script
# Create a closure that expects to input parameters named a and b
$add := {|$a $b| ($a + $b)}
# Outputs 3
add 1 2
# Outputs 7
add a=3 b=4
```

A closure that expects no parameters looks like `{|| ...}`.

You can declare the expected type of a parameterer using the syntax `: $type`, and a default value using 
the syntax` = $value`. Both can be combined, in which case the type must come before the default value.

```shell script
# Create a closure that expects input parameters named a and b, both integers. b has a default value of 1.
$add := {|$a :$integer $b :$integer = 1| ($a + $b)}
# Outputs 3
add 1 2
# Outputs 6
add 5
# Outputs 7
add a=6
# Error, no argument supplied for a
add b=3
# Error, wrong type flost for argument a
add 1.0
```

The type can be a simple value such as `$integer` or `$string`, or it can be a command substitution that returns a type,
such as `one_of $integer $float`. If the value provided is not a type, an error is emitted.

A parameter with a default can only be overridden by naming it (`$f 1 b=2`) -- a second
*positional* argument does not fill it in.

The default value of an argument is evaluated once, at the construction of the closure. This means that in this example,
every call to the closure adds a new element to the list `$l`.

```shell script
$push := {|$list = $(list:of 1)| $list:push 1; return $list}
# Returns a list with two elements
push 
# Returns a list with three elements
push 
# Returns a list with four elements
push 
```

The `@`/`@@` operators work in a closure's own parameter list, to collect stray arguments -- see
the section on [The `@` and `@@` operators](#the--and--operators).

#### Closure early termination using the `return` command

Closures can return early with the `return` command, which unwinds the innermost closure 
currently being called, and however many blocks are being executed inside that closure.

```shell script
{
    ||
    if $(check_early_exit) {
        return $false
    }
    ...
    $true
}
```

The output value of a closure that ends through a call to `return` is the value passed 
in to `return`. If none was given, the output value is `$empty`.

## Background jobs

A job started with a trailing `&` runs in the background: control returns to the script
immediately, and the job's eventual result is registered for later retrieval instead of
being waited for.

```shell script
$job_id := $(sleep $(duration:of seconds=2) &)
# ... do other work while it runs ...
fg $job_id
```

Only a *synchronous* failure to even start a backgrounded job (e.g. a bad argument)
surfaces immediately -- any error during its real work is deferred into the background
job registry and only reported once you `fg` it. A backgrounded job that fails and is
never `fg`'d has its failure go unreported.

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
crush# crush:warn:list
timestamp                 command message     file  location
2024-01-01 00:00:00 +0000 <block> value was 2 ...    ...
```

`each`, `where`, `group`, `files`, a class's `__init__`, and `remote:pexec` (a failed
host produces no row, not an error) all report into this log instead of failing
outright. `crush:warn:list` lists the most recent entries; `crush:warn:limit:get`/
`:set` control how many are kept (100 by default) before the oldest is evicted. In
interactive mode, a warning is also printed immediately as it happens; in a script, the
only way to notice one happened is to check `crush:warn:list` (or, for commands that
report per-item this way, to compare how much output you got against how much you
expected -- see e.g. `remote:pexec`'s own documentation).


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

A path written directly in command position runs as an external command too, without
needing to be found on `$PATH` first -- e.g. `./configure` or `~/bin/some-script`. If
the path is a directory and no arguments are given, it's `cd`'d into instead of
executed, the same as running `cd` directly. This only happens when the path is written
literally in command position (or referenced through a plain variable, e.g. `$f` where
`$f := ./configure`) -- a file path that merely *results* from evaluating something
else in that position (member access like `$s:script`, a dict/struct field read some
other way) is never executed, so reading a value that happens to hold a path can't
accidentally run it as a subprocess.
