# Crush overview

Crush is an attempt to make a traditional command line shell that is also a
modern programming language. It has the features one would expect from a modern
programming language like a type system, closures and lexical scoping, but with
a syntax geared toward both batch and interactive shell usage.

## What features of a traditional shell does Crush retain?

The basic structure of the Crush language resembles a regular shell like bash.

How to invoke commands, pass arguments and set up pipelines are unchanged, as is
the central concept of a current working directory. This means that trivial
invocations, like `ls` or `find .. | count` look the same, but under the hood
they are quite different, and nearly everything beyond that is different.

## What does Crush do so differently, then?

### Scratching the surface

Let's start with two trivial commands; listing files in the current directory,
and checking how many files are in the current directory:

    crush# files
    user size  modified                  type      file
    fox  2_279 2020-03-07 13:00:33 +0100 file      ideas
    fox  4_096 2019-11-22 21:56:30 +0100 directory target
    ...
    
    crush# files | count
    14

This all looks familiar. But appearances are deceiving. The `files` command being
called is a Crush builtin, and the output is not sent over a unix pipe but over
a Crush channel. It is not understood by the command as a series of bytes, but as
a table of rows, and Crush provides you with SQL-like commands to sort, filter,
aggregate and group rows of data.

    # Sort by size
    crush# files | sort size
    user size modified                  type      file
    fox    31 2019-10-03 13:43:12 +0200 file      .gitignore
    fox    75 2020-03-07 17:09:15 +0100 file      build.rs
    fox   491 2020-03-07 23:50:08 +0100 file      Cargo.toml
    fox   711 2019-10-03 14:19:46 +0200 file      crush.iml
    ...

    # Filter only directories
    crush# files | where {$type == directory}
    user size  modified                  type      file
    fox  4_096 2019-11-22 21:56:30 +0100 directory target
    fox  4_096 2020-02-22 11:50:12 +0100 directory tests
    fox  4_096 2020-03-16 14:11:39 +0100 directory .idea
    fox  4_096 2020-02-15 00:12:18 +0100 directory example_data
    fox  4_096 2020-03-14 17:34:39 +0100 directory src
    fox  4_096 2020-03-14 19:44:54 +0100 directory .git

Because Crush output is a stream of rows with columns, actions like sorting by
an arbitrary column or filtering data based on arbitrary logical expressions
operating on these columns is easy, and because the components used to do this
are generic and reusable, you can trivially do the same to data from any source,
such as a process list, a json file, an http request, etc.

### Reading and writing files

In traditional shells, I/O is done as binary streams. Because Crush streams
are typed, I/O happens differently. Crush has command pairs used
for serializing and deserializing various file formats. Use e.g. `json:from`
and `json:to` to deserialize and serialize json data, respectively. These
commands all work like you'd expect:

| Namespace | Description                                                    |
|-----------|----------------------------------------------------------------|
| `bin`     | Binary stream, i.e. no encoding at all.                        |
| `csv`     | Comma separated values. Only decoding supported.               |
| `json`    | JSON file format.                                              |
| `lines`   | Lines of text files.                                           |
| `pup`     | The native file format of Crush.                               |
| `split`   | Split text file on custom separators. Only decoding supported. |
| `toml`    | TOML file format.                                              |
| `words`   | Word split text files. Only decoding supported.                |
| `yaml`    | YAML file format.                                              |

```shell script
# Dump the output of the files command to the file listing.json in json format
crush# files | json:to ./listing.json

# Read the file Cargo.toml as a toml file, and extract the dependencies-field
crush# toml:from Cargo.toml | member dependencies

# Fetch a web page and write the body verbatim to a file
http "https://isitchristmas.com/" | member body | bin:to ./isitchristmas.html
```

If you don't supply an input file to any of the deserializer commands,
the command will read from the input, which in that case must be of type binary
or binary stream, e.g. `(http "https://jsonplaceholder.typicode.com/posts/1"):body | json:from`.

If you don't supply an output file to one of the serializer commands,
the command will serialize the output to a binary stream as the pipeline
output:

```shell script
crush# list:of "carrot" "carrot" "acorn" | json:to
[
  "carrot",
  "carrot",
  "acorn"
]
```

One of the Crush serializers, `pup`, is a native file format for Crush. The
Pup-format is protobuf-based, and its schema is available
[here](../src/crush.proto). The advantage of Pup is that all crush types,
including classes and closures, can be losslessly serialized into this format.
But because Pup is Crush-specific, it's useless for data sharing to
other languages.

### Expression mode

Crush allows you to perform mathematical calculations on integer and floating
point numbers directly in the shell using the same mathematical operators
used in almost any other programming language. To do so, you must enter a
seprate mode called "expression mode". You do so using parenthesis:

    crush# (5+6)
    11
    crush# (1+2*3)
    7

Comparisons between values are done using `>`, `<`, `<=`, `>=`, `==` and `!=`,
just like in most languages. All comparisons between values of different types result in an error.

    crush# (4 > 5)
    false
    crush# (40.0 > 5)
    Error: Values of type float and integer can't be compared with each other
    Error: receiving on an empty and disconnected channel

Expression mode is a fully functional secondary mode of crush. You invoke a command
using parenthesis, pipes still work, etc.

    # An entire pipeline written in command mode 
    (files() | sort("file", reverse=true) | where({size < 1000}))

### Conditional operators

The `and` and `or` operators are used to combine logical expressions:
    
    crush# $false or $true
    true
    crush# if $(./tree:exists) and {$((./tree:stat):is_file)} {echo "yay"}

### Globs and regular expressions
    
Crush also has operators related to patterns and matching. `=~` and `!~` are
used to check if a pattern matches an input:

    # The * character is the wildcard operator in globs
    crush# foo.txt =~ *.txt
    true

    # This is how you construct and match against a regular expression
    crush# abbbbbc =~ ^(ab+c")
    true

Regexps also support replacement using the `replace` and `replace_all` methods.

    crush# ^(a):replace tralala aaa
    traaalala
    crush# ^(a):replace_all tralala aaa
    traaalaaalaaa

### Type system

As already mentioned, many Crush commands operate on streams of tabular data.
The individual cells in this table stream can be any of a variety of types,
including strings, integers, floating point numbers, lists, binary data or
another table stream.

    crush# host:procs | head 5
    pid ppid status   user cpu  name
      1    0 Sleeping root 4.73 /sbin/init
      2    0 Sleeping root    0 [kthreadd]
      3    2 Idle     root    0 [rcu_gp]
      4    2 Idle     root    0 [rcu_par_gp]
      6    2 Idle     root    0 [kworker/0:0H-kblockd]

Some commands of course output a single value, such as pwd, which outputs the
current working directory as a single element of the `file` type.

### Variables of any type

Use the $ sigil to refer to variables. Variables must be declared
(using the `:=` operator) before use.

    crush# $some_number := 4      # The := operator declares a new variable
    crush# $some_number * 5
    20

Once declared, a variable can be reassigned to using the `=` operator.

    crush# $some_number = 6
    crush# $some_number * 5
    30

Like in any sane programming language, variables can be of any type supported by
the type system. There is no implicit type conversion. Do note that some
mathematical operators are defined between types, so multiplying an integer
with a floating point number results in a floating point number, for example.

    crush# some_text := "5"
    crush# some_text * some_number
    Error: Can not process arguments of specified type

Variable names beginning with double underscores (`__`) are reserved for internal
use by Crush. They can not be assigned to.

### Named and unnamed arguments

Crush commands support named and unnamed arguments. It is often possible to use one,
the other or a combination of both. The following three invocations are equivalent.

    http uri="http://example.com" method=get
    http "http://example.com" get
    http "http://example.com" method=get

It is quite common to want to pass boolean arguments to commands, which is why
Crush has a special shorthand syntax for it. Using one or two leading dashes, like `--foo` or `-foo`
is equivalent to `foo=$true`.

### Subshells

Sometimes you want to use the output of one command as an *argument* to another
command, just like a subshell in e.g. bash. This is different from what a pipe does,
which is using the output as the *input*. To do this, use the so called subshell syntax by putting
the command within dollar-parenthesis (`$()`), like so:

    crush# echo $(pwd)

### Blocks

In Crush, braces (`{}`) are used to create blocks of code. 

Any named arguments passed when calling a block are added to the local scope
of the invocation:

    crush# $print_a := {echo $a}
    crush# print_a a="Greetings"
    Greetings

#### Closures

For added type safety, you may optionally declare what parameters a block of code accepts.
Such blocks are called closures:

    crush# {|$a $b $c| echo $a $b $c}

Closures also allow you to break execution early using the return builtin command:

    {
        ||
        if $(check_early_exit) { 
            # This call will return the entire closure, not just the innermost block
            return
        }

        ...
    }

Assign a closure or a block to a variable in order to create your own custom commands

    crush# $print_greeting := {echo "Hello!"}
    crush# print_greeting
    Hello!

You can specify the type of a closure argument and/or the default value using the
following syntax:

    # 'b' must be an integer number, and if unspecified, will have the value 7 
    crush# print_things := {|$a $b: $integer = 7|}

Additionally, the `@` operator can be used to create a list of all unnamed
arguments, and the `@@` operator can be used to create a dict of all named
arguments not mentioned elsewhere in the parameter list.

    crush# print_everything := {|@ $unnamed @@ $named| echo "Named" $named "Unnamed" $unnamed}

The `@` and `@@` operators are also used during command invocation to perform
the mirrored operation. The following code creates an `ls` function that calls
the `files` command and passes on any arguments to it, and pipes the output through
the `select` command to only show one column from the output.

    $ls := {|@ $args @@ $kwargs| files @ $args @@ $kwargs| select file}

### Types

Crush comes with a variety of types:

* lists of any type,
* dicts of a pair of types, (Some types can not be used as keys!)
* strings,
* regular expressions,
* globs,
* files,
* booleans,
* integer numbers,
* floating point numbers,
* structs, which contain any number of named fields of any type,
* tables, which are essentially lists where each element is the same type of struct,
* table input/output streams, which are like tables but can only be traversed once,
* binary data,
* binary streams, which are like binary data but can only be traversed once,
* types, and
* commands, which are either closures or built in commands.

Crush allows you to create your own types using the `class` and `data`
commands.

### Exploring the shell

When playing around with Crush, the `help` and `dir`commands are useful. The
former displays a help messages, the latter lists the content of a value.

    crush# help $sort
    sort [field=string...] [reverse=bool]

        Sort input based on column

        Output: A stream with the same columns as the input

        This command accepts the following arguments:

        * field, the columns to sort on. Optional if input only has one column.

        * reverse (false), reverse the sort order.

        Example

        host:procs | sort cpu

### Namespaces, members and methods

Members are accessed using the `:` operator. Most other languages tend to use
`.`, but that is a very common character in file names, so Crush needed to use
something else.

Most types have several useful methods. Files have `exists` and `stat`, which do
what you'd expect.

    crush# .:exists
    true
    crush# .:stat
    is_directory: true
    is_file:      false
    is_symlink:   false
    inode:        50_856_186
    nlink:        11
    mode:         16_877
    len:          4_096

### Semi-lazy stream evaluation:

If you assign the output of the find command to a variable like so:

    crush# $all_the_files := $(files --recurse /)

What will really be stored in the `all_the_files` variable is simply a stream. A
small number of lines of output will be eagerly evaluated, before the thread
executing the find command will start blocking. If the stream is consumed, for
example by writing

    crush# $all_the_files

then all hell will break loose on your screen as tens of thousands of lines are
printed to your screen.

Another option would be to pipe the output via the head command

    crush# $all_the_files | head 1

Which will consume one line of output from the stream. This command can be
re-executed until the stream is empty.

### More SQL-like data stream operations

Crush features many commands to operate om arbitrary streams of data using a
SQL-like syntax:

    host:procs | where {$user == root} | group status proc_per_status={count} | sort proc_per_status
    status   proc_per_status
    Idle     108
    Sleeping 170

Unlike in SQL, these commands all operate on input streams, meaning they can be
combined in any order, and the input source can be file/http resources in a
variety of formats or output of commands like `host:procs`, or `files`.

### Globs

As other shells, Crush uses `*` as the wildcard operator and `?` for single character wildcards.
The operator `**` is used for performing globbing recursively into subdirectories.

    crush# files *.md
    permissions links user group size      modified                  type file
    rw-r--r--       1 fox  fox   1.643 kiB 2022-09-21 16:07:19 +0200 file README.md
    crush# files ????????
    permissions links  user group size  modified                  type file
    rw-r --r--       1 fox  fox   150 B 2024-07-22 15:18:45 +0200 file build.rs
    # Count the number of lines of rust code in this tree
    crush# lines:from **.rs | count
    114552

Wildcards are not automatically expanded, they are passed in to commands as glob
objects, and the command chooses what to match the glob against. If you want to
perform glob expansion in a command that doesn't do so itself, use the `:files`
method of the glob object to do so:

    crush# **.rs:files

### Regular expressions

Regular expressions are constructed like `^(REGEXP GOES HERE)`. They support
matching and replacement:

    crush# abbbbbc =~ ^(ab+c)
    true
    crush# ^(a+):replace baalaa a
    balaa
    crush# ^(a+):replace_all baalaa a
    bala

### Lists and dicts

Crush has built-in lists:

    crush# $l := $(list:of 1 2 3)
    crush# $l
    [1, 2, 3]
    crush# $l:peek
    3
    crush# $l:pop
    3
    crush# $l:len
    2
    crush# $l[1]
    2
    crush# $l[1] = 7
    crush# $l
    [1, 7]
    crush# help $l
    type list integer

    A mutable list of items, usually of the same type

    * __call__     Returns a list type with the specified value type.
    * __getitem__  Return the value at the specified index of the list.
    * __setitem__  Assign a new value to the element at the specified index.
    * clear        Remove all values from this list.
    [...]

and dictionaries:

    crush# $d := (dict string integer):new
    crush# $d["foo"] = 42
    crush# $d["foo"]
    42
    crush# help $d
    type dict string integer

        A mutable mapping from one set of values to another

        * __call__  Returns a dict type with the specifiec key and value types
        * __getitem__    Return the value the specified key is mapped to
        * __setitem__    Create a new mapping or replace an existing one
        * clear          Remove all mappings from this dict
        [...]

### Time

Crush has two data types for dealing with time: `time` and `duration`.

    crush# $start := $(time:now)
    crush# $something_that_takes_a_lot_of_time
    crush# $end := $(time:now)
    crush# echo ("We spent {} on the thing":format (end - start))
    4:06

The mathematical operators that make sense are defined for `time` and
`duration`. Subtracting one `time` from another results in a `duration`. Adding
two `duration` results in a `duration`. Multiplying or dividing a `duration` by
a `integer` results in a `duration`.

### Materialized data

The output of many commands is a table stream, i.e. a streaming data structure
consisting of rows with identical structure. Some commands, like `cat` instead
output a binary stream.

These streams can not be rewound and can only be consumed once. This is
sometimes vital, as it means that one can work on data sets larger than your
computers memory, and even infinite data sets. It also allows for parallel execution
of different steps in the pipeline, which improves performance.

But sometimes, streaming data sets are inconvenient, especially if one wants to
use the same dataset twice.

    crush# $f := $(files)
    crush# $f
    permissions links user group size      modified                  type      file
    rw-r--r--       1 fox  staff 1.792 kiB 2024-08-09 13:46:03 +0200 file      Cargo.toml
    rw-r--r--       1 fox  staff 1.037 kiB 2022-08-24 17:41:16 +0200 file      LICENSE
    rwxr-xr-x       4 fox  staff     128 B 2022-09-21 22:15:38 +0200 directory signature
    rwxr-xr-x       7 fox  staff     224 B 2025-05-15 13:37:58 +0200 directory target
    crush# $f
    crush#

Notice how there is no output the second time the content of the `$f` variable is
displayed, because the table_input_stream has already been consumed.

Enter the materialize command, which takes any value and recursively converts
all transient (table_input_stream and binary_stream) components into an equivalent
in-memory form (table, and binary, respectively).

    crush# $materialized_files := (files|materialize)
    crush# $materialized_files
    permissions links user group size      modified                  type      file
    rw-r--r--       1 fox  staff 1.792 kiB 2024-08-09 13:46:03 +0200 file      Cargo.toml
    rw-r--r--       1 fox  staff 1.037 kiB 2022-08-24 17:41:16 +0200 file      LICENSE
    rwxr-xr-x       4 fox  staff     128 B 2022-09-21 22:15:38 +0200 directory signature
    rwxr-xr-x       7 fox  staff     224 B 2025-05-15 13:37:58 +0200 directory target
    crush# $materialized_files
    permissions links user group size      modified                  type      file
    rw-r--r--       1 fox  staff 1.792 kiB 2024-08-09 13:46:03 +0200 file      Cargo.toml
    rw-r--r--       1 fox  staff 1.037 kiB 2022-08-24 17:41:16 +0200 file      LICENSE
    rwxr-xr-x       4 fox  staff     128 B 2022-09-21 22:15:38 +0200 directory signature
    rwxr-xr-x       7 fox  staff     224 B 2025-05-15 13:37:58 +0200 directory target
    crush#

When the `table_input_stream` is materialized into a `table`, it can be accessed
multiple times.

### Flow control

Of course Crush has an `if` command, as well as `for`, `while` and `loop` loops,
that can be controlled using `break` and `continue`.

    crush# help if
    if condition:bool if-clause:command [else-clause:command]
    
        Conditionally execute a command once.
    
        If the condition is true, the if-clause is executed. Otherwise, the else-clause
        (if specified) is executed.
    
        Example:
    
        if $(./some_file:stat:is_file) {echo "It's a file!"} {echo "It's not a file!"}


    for [name=]iterable:(table_input_stream|table|dict|list) body:command
    
        Execute body once for every element in iterable.
    
        Example:
    
        for $(seq) {
            echo ("Lap {}":format value)
        }


### Calling external commands

Obviously, one needs to sometimes call out to external commands. Currently, the
functionality for doing so in Crush is somewhat primitive. If an internal
command of a given name does not exist, Crush looks for external commands, and
if one is found, it is used. But Crush does not hand over the tty or emulate a
tty, so interactive terminal programs do not work, and commands that prettify
their output with escape sequences may fail.

This part of Crush should be considered a proof of concept, but still, most
non-interactive commands work as expected:

    crush# whoami
    fox
    
Crush features several shortcuts to make working with external commands easier.

* Named arguments are transparently translated into options. Single
  character argument names are turned into options with a single hyphen, and
  multi-character argument names are turned into GNU style long options with
  two hyphens, e.g. `git commit m="hello"` is converted into 
  `git commit -m "hello"` and `git commit message="hello"` is converted into
  `git commit --message "hello"`.
* Thirdly, named arguments with a value of boolean true are simply turned into
  options without a value, so for example `git commit --a --append` (or 
  `git commit a=true append=true` for that matter) is converted into
  `git commit -a --append`.

Further work is required when it comes to job control, terminal emulation and various
other integration points.

### Executing commands remotely or as other users

Traditional shells allow you to run commands as other users or on
other systems using commands like ssh or sudo. The problem with
their approach is that the commands to run and their parameters are
locally expanded and then transferred as text. This leads to a multitude
of issues related to double expansion, double whitespace splitting and
permissions on I/O redirections. Work around these problems leads to
quoting, escaping, double quoting, double escaping, hair loss, insanity
and if unmitigated, eventually suicide.

The Crush way of running commands in other processes (potentially on
other machines) is to pass in a closure as an argument to the command.
The command will serialize the closure, transfer it to the remote
process, and run the closure remotely. The output of this remote
execution is then serialized and passed back to
the calling process.

To execute a command as another user, use the `do` method of the
user you want to do something as:

    user[root]:do {./carrot:chown group="rabbit"}

To execute a command on a remote host, use the `remote:exec` command:

    remote:exec {uptime} "popplar.meadow"

To run a closure on multiple remote hosts, use `remote:pexec` instead.

### Creating custom types

You can create custom types in Crush, by using the class command:

    $Point := (class)

    $Point:__init__ = {
        |$x:$float $y:$float|
        $this:x = $x
        $this:y = $y
    }

    $Point:len = {
        ||
        math:sqrt $this:x*$this:x + $this:y*$this:y
    }

    $Point:__add__ = {
        |$other|
        Point:new x=($this:x + $other:x) y=($this:y + $other:y)        
    }

    $p := (Point:new x=1.0 y=2.0)
    $p:len

Crush supports single inheritance (by passing in the parent to the class
command). The class command will create a new struct, that contains a method
named `new`. When called, `new` will create a new instance of the class. If the
`__init__` method is defined, `new` will call it, and pass on any parameters to
it.

Add methods by adding them to the class, add member variables by adding them to
the instance (`this`) in `__init__`.

## Summary

Hopefully, that is enough to give a good sense of what problems Crush is trying to
solve, and if the project is of interest to you.
