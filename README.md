# Crush

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

    crush> ls
    user         size modified                  type      file
    liljencrantz 2279 2020-03-07 13:00:33 +0100 file      ideas
    liljencrantz 4096 2019-11-22 21:56:30 +0100 directory target
    ...
    
    crush> ls | count
    14

This all looks familiar. But appearances are deceiving. The `ls` command being
called is a Crush builtin, and the output is not sent over a unix pipe but over
a Rush channel. It is not understood by the command as a series of bytes, but as
a table of rows, and Crush provides you with SQL-like commands to sort, filter,
aggregate and group rows of data.

    crush> ls | sort ^size
    user         size  modified                  type      file
    liljencrantz    31 2019-10-03 13:43:12 +0200 file      .gitignore
    liljencrantz    75 2020-03-07 17:09:15 +0100 file      build.rs
    liljencrantz   491 2020-03-07 23:50:08 +0100 file      Cargo.toml
    liljencrantz   711 2019-10-03 14:19:46 +0200 file      crush.iml
    ...

    crush> ls | where {type == "directory"}
    user         size modified                  type      file
    liljencrantz 4096 2019-11-22 21:56:30 +0100 directory target
    liljencrantz 4096 2020-02-22 11:50:12 +0100 directory tests
    liljencrantz 4096 2020-03-16 14:11:39 +0100 directory .idea
    liljencrantz 4096 2020-02-15 00:12:18 +0100 directory example_data
    liljencrantz 4096 2020-03-14 17:34:39 +0100 directory src
    liljencrantz 4096 2020-03-14 19:44:54 +0100 directory .git

Because Crush output is a stream of rows with columns, actions like sorting by
an arbitrary column or filtering data based on arbitrary logical expressions
operating on these columns is easy, and because the components used to do this
are generic and reusable, you can trivially do the same to data from any source,
such as json files, http requests, etc.

### Reading and writing files

In traditional shells, I/O is done as binary streams. Because Crush streams
are typed, I/O happens differently. Crush has command pairs used
for serializing and deserializing various file formats. Use e.g. `json:from`
and `json:to` to deserialize and serialize json data, respectively. These
commands all work like you'd expect:

| Namespace | Description |
| --- | --- |
| `bin` | Binary stream, i.e. no encoding at all. |
| `csv` | Comma separated values. |
| `json` | JSON file format. |
| `lines` | Lines of text files. |
| `pup` | The native file format of Crush.  |
| `split` | Split text file on custom separators. |
| `toml` | TOML file format. |
| `words` | Word split text files. |

```shell script
# Dump the output of the ls command to the file listing.json in json format
crush> ls | json:to ./listing.json

# Read the file Cargo.toml as a toml file, and extract the dependencies-field
crush> (toml:from Cargo.toml):dependencies

# Fetch a web page and write it to a file
(http "https://isitchristmas.com/"):body | bin:to ./isitchristmas.html
```

If you don't supply an input file to any of the deserializer commands,
the command will read from the input, which must be a binary or binary
stream, e.g. `(http "https://jsonplaceholder.typicode.com/posts/1"):body | json:from`.

If you don't supply an output file to one of the serializer commands,
the command will serialize the output to a binary stream as the pipeline
output:

```shell script
crush> list:of 1 2 3 | json:to
[1,2,3]
```

One of the Crush serializers, Pup, is a native file format for Crush. The
Pup-format is protobuf-based, and its schema is available
[here](src/crush.proto). The advantage of Pup is that all crush types,
including classes and closures, can be losslessly serialized into this format.
But because Pup is Crush-specific, it's useless for data sharing to
other languages.

### Operators for comparison, logical operations and arithmetical operations

Crush allows you to perform mathematical calculations on integer and floating
point numbers directly in the shell, mostly using the same mathematical operators
used in almost any other programming language.

    crush> 5+6
    11
    crush> 1+2*3
    7

The only exception is that the `/` operator is used for constructing files and
paths (more on that later), so division is done using the `//` operator.

    crush> 4.2//3
    1.4000000000000001

Comparisons between values are done using `>`, `<`, `<=`, `>=`, `==` and `!=`,
just like in most languages. All comparisons between values of different types
are false.

    crush> 4 > 5
    false
    
The `and` and `or` operators are used to combine logical expressions:
    
    crush> false or true
    true
    crush> if some_file:exists and (some_file:stat):is_file {echo "yay"}
    
Crush also has operators related to patterns and matching. `=~` and `!~` are
used to check if a pattern matches an input:

    # The % character is the wildcard operator in globs
    crush> %.txt =~ foo.txt
    true
    # This is how you construct and match a regular expression
    crush> re"ab+c" =~ "abbbbbc"
    true

Regexps also support replacement using the `~` (replace once) and `~~` (replace
all) operators, which are trinary operators:

    crush> re"a+" ~ "baalaa" "a"
    balaa
    crush> re"a+" ~~ "baalaa" "a"
    bala


### Type system

As already mentioned, many Crush commands operate on streams of tabular data.
The individual cells in this table stream can be any of a variety of types,
including strings, integers, floating point numbers, lists, binary data or
another table stream.

    crush> ps | head 5
    pid ppid status   user cpu  name
      1    0 Sleeping root 4.73 /sbin/init
      2    0 Sleeping root    0 [kthreadd]
      3    2 Idle     root    0 [rcu_gp]
      4    2 Idle     root    0 [rcu_par_gp]
      6    2 Idle     root    0 [kworker/0:0H-kblockd]

Some commands of course output a single value, such as pwd, which outputs the
current working directory as a single element of the `file` type.

### Variables of any type

Variables must be declared (using the `:=` operator) before use.

    crush> some_number := 4      # The := operator declares a new variable
    crush> some_number * 5
    20

Once declared, a variable can be reassigned to using the `=` operator.

    crush> some_number = 6
    crush> some_number * 5
    30

Like in any sane programming language, variables can be of any type supported by
the type system. There is no implicit type conversion. Do note that some
mathematical operators are defined between types, so multiplying an integer
with a floating point number results in a floating point number, for example.

    crush> some_text := "5"
    crush> some_text * some_number
    Error: Can not process arguments of specified type

### Named and unnamed arguments

Crush supports named and unnamed arguments. It is often possible to use one,
the other or a combination of both. The following three invocations are equivalent.

    http uri="http://example.com" method="get"
    http "http://example.com" "get"
    http "http://example.com" method="get"

It is quite common to want to pass boolean arguments to commands, which is why
Crush has a special shorthand syntax for it. Passing in `--foo` is equivalent
to passing in `foo=true`.

### Subshells

Sometimes you want to use the output of one command as an *argument* to another
command, just like a subshell in e.g. bash. This is different from using the
output as the *input*, and is done using `()`:

    crush> echo (pwd)

### Closures

In Crush, braces (`{}`) are used to create a closure. Assigning a closure to a
variable is how you create a function.

    crush> print_greeting := {echo "Hello"}
    crush> print_greeting
    Hello

Any named arguments passed when calling a closure and added to the local scope
of the invocation:

    crush> print_a := {echo a}
    crush> print_a a="Greetings"
    Greetings

For added type safety, you can declare what parameters a closure expects at the
start of a closure.

The following closure requires the caller to supply the argument `a`, and allows
the caller to specify the argument `b`, which must by of type integer. If the
caller does not specify it, it falls back to a default value of 7.

    crush> print_things := {|a b: integer = 7|}

Additionally, the `@` operator can be used to create a list of all unnamed
arguments, and the `@@` operator can be used to create a list of all named
arguments not mentioned elsewhere in the parameter list.

    crush> print_everything := {|@unnamed @@named| echo "Named" named "Unnamed" unnamed}

The `@` and `@@` operators are also used during command invocation to perform
the mirrored operation. The following code creates an `lss` function that calls
the `ls` command and passes on any arguments to it, and pipes the output through
the `select` command to only show one column from the output.

    lss := {|@args @@kwargs| ls @args @@kwargs | select %file}

### Types

Crush comes with a variety of types:

* lists of any type,
* dicts of any pair of types type, (Some types can not be used as keys!)
* strings,
* regular expressions,
* globs,
* files,
* booleans,
* integer numbers,
* floating point numbers,
* structs, which contain any number of named fields of any type,
* tables, which are essentially lists where each element is the same type of struct,
* table streams, which are like tables but can only be traversed once,
* binary data,
* binary streams, which are like binary data but can only be traversed once,
* types, and
* commands, which are either closures or built in commands.

Crush allows you to create your own types using the `class` and `data`
commands.

### Exploring the shell

When playing around with Crush, the `help` and `dir`commands are useful. The
former displays a help messages, the latter lists the content of a value.

    crush> help
    sort column:field
    
        Sort input based on column
    
        Example:
    
        ps | sort ^cpu
    crush> dir list
    [type, truncate, remove, clone, of, __call_type__, __setitem__, pop, push, empty, len, peek, new, clear]

### The content of your current working directory lives in your namespace

All the files in the current working directory are part of the local namespace.
This means that e.g. `.` is a file object that points to the current working
directory. The `/` operator is used in Crush to join two file directory element
together.

This means that for the most part, using files in Crush is extremely simple and
convenient.

    crush> cd .. # This does what you'd think
    crush> cd /  # As does this

The right hand side of the / operator is a label, not a value, so `./foo` refers
to a file named foo in the current working directory, and is unrelated to the
contents of any variable named `foo`.

### Namespaces, members and methods

Members are accessed using the `:` operator. Most other languages tend to use
`.`, but that is a very common character in file names, so Crush needed to find
something else.

Most types have several useful methods. Files have `exists` and `stat`, which do
what you'd expect.

    crush> .:exists
    true
    crush> .:stat
    {is_directory: true, is_file: false, is_symlink: false, inode: 50856186, nlink: 8, mode: 16877, len: 4096}
    crush> (.:stat):is_file
    false

### Semi-lazy stream evaluation:

If you assign the output of the find command to a variable like so:

    crush> all_the_files := (find /)

What will really be stored in the `all_the_files` variable is simply a stream. A
small number of lines of output will be eagerly evaluated, before the thread
executing the find command will start blocking. If the stream is consumed, for
example by writing

    crush> all_the_files

then all hell will break loose on your screen as tens of thousands of lines are
printed to your screen.

Another option would be to pipe the output via the head command

    crush> all_the_files | head 1

Which will consume one line of output from the stream. This command can be
re-executed until the stream is empty.

### More SQL-like data stream operations

Crush features many commands to operate om arbitrary streams of data using a
SQL-like syntax. These commands use field-specifiers like `^foo` to specify
columns in the data stream that they operate on:

    ps | where {user == "root"} | group ^status | aggr proc_per_status={count}

(Note that the `aggr` command is currently broken.)

Unlike in SQL, these commands all operate on input streams, meaning they can be
combined in any order, and the input source can be file/http resources in a
variety of formats or output of commands like `ps`, `find`.

### Globs

The `*` operator is used for multiplication, so Crush uses `%` as the wildcard
operator instead. `?` is still used for single character wildcards.

    crush> ls %.txt
    user         size  modified                  type file
    liljencrantz 21303 2020-03-30 13:40:37 +0200 file /home/liljencrantz/src/crush/README.md
    crush> ls ????????
    user         size modified                  type file
    liljencrantz   75 2020-03-07 17:09:15 +0100 file /home/liljencrantz/src/crush/build.rs

The operator `%%` is used for performing globbing recursively into subdirectories.
Another way of looking ath the same syntax is to say that `%` and `?` match any
character except `/`, whereas `%%` also matches `/`.

    # Count the number of lines of rust code in the crush source code
    crush> lines src/%%.rs|count

Wildcards are not automatically expanded, they are passed in to commands as glob
objects, and the command chooses what to match the glob against. If you want to
perform glob expansion in a command that doesn't do so itself, use the `:files`
method of the glob object to do so:

    crush> echo (%%.rs):files

### Regular expressions

Regular expressions are constructed like `re"REGEXP GOES HERE"`. They support
matching and replacement:

    crush> re"ab+c" =~ "abbbbbc"
    true
    crush> re"a+" ~ "baalaa" "a"
    balaa
    crush> re"a+" ~~ "baalaa" "a"
    bala

### Lists and dicts

Crush has built-in lists:

    crush> l := (list:of 1 2 3)
    crush> l
    [1, 2, 3]
    crush> l:peek
    3
    crush> l:pop
    3
    crush> l:len
    2
    crush> l[1]
    2
    crush> l[1] = 7
    crush> l
    [1, 7]
    crush> help l
    type list integer

        A mutable list of items, usually of the same type

        * __call_type__  Return a list type for the specified element type
        * __getitem__    Return a file or subdirectory in the specified base directory
        * __setitem__    Assign a new value to the element at the specified index
        * clear          Remove all elments from the list
        * clone          Create a duplicate of the list
        * empty          True if there are no elements in the list
        * len            The number of elements in the list
        * new            Create a new list with the specified element type
        * of             Create a new list containing the supplied elements
        * peek           Return the last element from the list
        * pop            Remove the last element from the list
        * push           Push an element to the end of the list
        * remove         Remove the element at the specified index
        * truncate       Remove all elements past the specified index

and dictionaries:

    crush> d := (dict string integer):new
    crush> d["foo"] = 42
    crush> d["foo"]
    42
    crush> help d
    type dict string integer

        A mutable mapping from one set of values to another

        * __call_type__  Returns a dict type with the specifiec key and value types
        * __getitem__    Return the value the specified key is mapped to
        * __setitem__    Create a new mapping or replace an existing one
        * clear          Remove all mappings from this dict
        * clone          Create a new dict with the same st of mappings as this one
        * empty          True if there are no mappings in the dict
        * len            The number of mappings in the dict
        * new            Construct a new dict
        * remove         Remove a mapping from the dict

### Time

Crush has two data types for dealing with time: `time` and `duration`.

    crush> start := time:now
    crush> something_that_takes_a_lot_of_time
    crush> end := time:now
    crush> echo ("We spent {} on the thing":format end - start)
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
computers memory, and even infinite data sets.

But sometimes, streaming data sets are inconvenient, especially if one wants to
use the same dataset twice.

    crush> files := ls
    crush> files
    user         size  modified                  type      file
    liljencrantz  1307 2020-03-26 01:08:45 +0100 file      ideas
    liljencrantz  4096 2019-11-22 21:56:30 +0100 directory target
    liljencrantz  4096 2020-03-27 09:18:25 +0100 directory tests
    liljencrantz 95328 2020-03-24 17:20:00 +0100 file      Cargo.lock
    liljencrantz  4096 2020-02-15 00:12:18 +0100 directory example_data
    liljencrantz    31 2019-10-03 13:43:12 +0200 file      .gitignore
    liljencrantz 13355 2020-03-29 03:05:16 +0200 file      README.md
    liljencrantz  4096 2020-03-27 11:35:25 +0100 directory src
    liljencrantz   479 2020-03-24 17:20:00 +0100 file      Cargo.toml
    liljencrantz  4096 2020-03-29 01:29:52 +0100 directory .git
    liljencrantz  8382 2020-03-29 00:54:13 +0100 file      todo
    liljencrantz    75 2020-03-07 17:09:15 +0100 file      build.rs
    liljencrantz   711 2019-10-03 14:19:46 +0200 file      crush.iml
    crush> files

Notice how there is no output the second time `files` is displayed, because the
table_stream has already been consumed.

Enter the materialize command, which takes any value and recursively converts
all transient components into an equivalent but fully in-memory form.

    crush> materialized_files := (ls|materialize)
    crush> materialized_files
    user         size  modified                  type      file
    liljencrantz  1307 2020-03-26 01:08:45 +0100 file      ideas
    liljencrantz  4096 2019-11-22 21:56:30 +0100 directory target
    liljencrantz  4096 2020-03-27 09:18:25 +0100 directory tests
    liljencrantz 95328 2020-03-24 17:20:00 +0100 file      Cargo.lock
    liljencrantz  4096 2020-02-15 00:12:18 +0100 directory example_data
    liljencrantz    31 2019-10-03 13:43:12 +0200 file      .gitignore
    liljencrantz 14420 2020-03-29 03:06:02 +0200 file      README.md
    liljencrantz  4096 2020-03-27 11:35:25 +0100 directory src
    liljencrantz   479 2020-03-24 17:20:00 +0100 file      Cargo.toml
    liljencrantz  4096 2020-03-29 01:29:52 +0100 directory .git
    liljencrantz  8382 2020-03-29 00:54:13 +0100 file      todo
    liljencrantz    75 2020-03-07 17:09:15 +0100 file      build.rs
    liljencrantz   711 2019-10-03 14:19:46 +0200 file      crush.iml
    crush> materialized_files
    user         size  modified                  type      file
    liljencrantz  1307 2020-03-26 01:08:45 +0100 file      ideas
    liljencrantz  4096 2019-11-22 21:56:30 +0100 directory target
    liljencrantz  4096 2020-03-27 09:18:25 +0100 directory tests
    liljencrantz 95328 2020-03-24 17:20:00 +0100 file      Cargo.lock
    liljencrantz  4096 2020-02-15 00:12:18 +0100 directory example_data
    liljencrantz    31 2019-10-03 13:43:12 +0200 file      .gitignore
    liljencrantz 14420 2020-03-29 03:06:02 +0200 file      README.md
    liljencrantz  4096 2020-03-27 11:35:25 +0100 directory src
    liljencrantz   479 2020-03-24 17:20:00 +0100 file      Cargo.toml
    liljencrantz  4096 2020-03-29 01:29:52 +0100 directory .git
    liljencrantz  8382 2020-03-29 00:54:13 +0100 file      todo
    liljencrantz    75 2020-03-07 17:09:15 +0100 file      build.rs
    liljencrantz   711 2019-10-03 14:19:46 +0200 file      crush.iml

When the `table_stream` is materialized into a `table`, it can be displayed
multiple times.

### Flow control

Of course Crush has an `if` command, as well as `for`, `while` and `loop` loops,
that can be controlled using `break` and `continue`.

    crush> help if
    if condition:bool if-clause:command [else-clause:command]
    
        Conditionally execute a command once.
    
        If the condition is true, the if-clause is executed. Otherwise, the else-clause
        (if specified) is executed.
    
        Example:
    
        if (./some_file:stat):is_file {echo "It's a file!"} {echo "It's not a file!"}


    for [name=]iterable:(table_stream|table|dict|list) body:command
    
        Execute body once for every element in iterable.
    
        Example:
    
        for (seq) {
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

    crush> whoami
    liljencrantz
    
Crush features several shortcuts to make working with external commands easier.

* Firstly, subcommands like `git status` are mapped into method calls like
`git:status`. That way you do not have to quote the subcommand name, e.g.
`git "status"`.
* Secondly, named arguments are transparently translated into options. Single
  character argument names are turned into options with a single hyphen, and
  multi-character argument names are turned into GNU style long options with
  two hyphens, e.g. `git:commit m="hello"` is converted into 
  `git commit -m "hello"` and `git:commit message="hello"` is converted into
  `git commit --message "hello"`.
* Thirdly, named arguments with a value of boolean true are simply turned into
  options without a value, so for example `git:commit --a --append` (or 
  `git:commit a=true append=true` for that matter) is converted into
  `git commit -a --append`.

Further work is required when it comes to job control, terminal emulation and various
other integration points.

### Executing remote commands

To run a closure on a remote host, use the `remote:exec` command:

    remote:exec {uptime} "example.com"

The closure will be serialized, transferred to the remote host using
ssh, deserialized, and executed on the remote host (the crush shell
must be in the default path on the remote host). Once the command has
been executed and the output of the closure is serialized, transferred,
deserialized on the local machine and used as the output of the
`remote:exec` command.

To run a closure on multiple remote hosts, use `remote:pexec` instead.

### Creating custom types

You can create custom types in Crush, by using the class command:

    Point := (class)

    Point:__init__ = {
        |x:float y:float|
        this:x = x
        this:y = y
    }

    Point:len = {
        ||
        math:sqrt this:x*this:x + this:y*this:y
    }

    Point:__add__ = {
        |other|
        Point:new x=(this:x + other:x) y=(this:y + other:y)        
    }

    p := (Point:new x=1.0 y=2.0)
    p:len

Crush supports single inheritance (by passing in the parent to the class
command). The class command will create a new struct, that contains a method
named `new`. When called, `new` will create a new instance of the class. If the
`__init__` method is defined, `new` will call it, and pass on any parameters to
it.

Add methods by adding them to the class, add member variables by adding them to
the instance (`this`) in `__init__`.

## Similarity to PowerShell

Crush shares the majority of its design goals with PowerShell. I consider
PowerShell one of the coolest and most interesting innovations to ever come out
of Microsoft. That said, I've found using PowerShell in practice to often feel
clunky and annoying, especially for interactive use. I also feel that tying a
shell to COM objects is a poor fit.

I wanted to do something similar but with a more streamlined syntax, and with
what I felt was a more suitable type system.

## Similarity to Nushell

On the surface, Crush looks identical to nushell, but less polished. Crush lacks
syntax highlighting, tab completion and has a worse screen rendering. But that
is because the focus of Crush right now is to create a well defined, powerful
and convenient language that supports things like arithmetic operations,
closures, loops and flow control while remaining useful for interactive use.

### Future work

There are plenty of langage ideas waiting to be tried out. Pattern matching and
error handling are among the most obvious. Also, the error handling in Crush
itself is currently very rudimentary.

## About the codebase

I am teaching myself rust by writing Crush. I still have plenty to learn. :-)

## Building and installing Crush

Crush should work on any modern Unix system. Install rust,
 
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

clone this repository,

    git clone https://github.com/liljencrantz/crush.git
 
and run

    cd crush; cargo build

and you should have a working binary to try out.

Have fun!
