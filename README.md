# Crush

Crush is an attempt to create a command line shell that adds a type system,
closures and a more versatile syntax to a traditional shell environment without
sacrificing the usefulness of a traditional shell.

## What features of a traditional shell does Crush retain

How to invoke commands, pass arguments and set up pipelines are unchanged, as is the central
concept of a current working directory .

Trivial invocations, like `ls` or `find ..` work identically, but from there on, most things
are different.

## What does Crush do differently

### Some examples

Listing files in the current directory works the same as in any other shell.

    crush> ls
    user         size modified                  type      file
    liljencrantz 2279 2020-03-07 13:00:33 +0100 file      ideas
    liljencrantz 4096 2019-11-22 21:56:30 +0100 directory target
    ...
    
    crush> ls | count
    14

But ls output is a table of rows, so we use SQL-like commands to sort, filter and group lines.

    crush> ls | sort %size
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

### Operators for comparison, logical operations and arithmetical operations

    crush> 5+6
    11
    crush> 4 > 5
    false
    crush> false and true
    false

### Type system

The input and output of Crush pipelines aren't streams of bytes, they can be any type in a rich type
system. The most common input/output type is a stream of rows, where each row consists of the columns, such
as from the `ps` command:

    crush> ps | head 5
    pid ppid status   user cpu  name
      1    0 Sleeping root 4.73 /sbin/init
      2    0 Sleeping root    0 [kthreadd]
      3    2 Idle     root    0 [rcu_gp]
      4    2 Idle     root    0 [rcu_par_gp]
      6    2 Idle     root    0 [kworker/0:0H-kblockd]

But other commands output a single value, such as pwd, which outputs the current working directory
as a single element of the `file` type.

### Variables of any type

    crush> some_number := 4
    crush> some_number * 5
    20
    crush> some_text := "hello"
    crush> some_text * some_number
    Error: Can not process arguments of specified type

### Closures

In crush, braces (`{}`) re used to create a closure. To create a version of the `ls` command that only shows you
file names, simply write

lss := {|@args @@kwargs| ls @args @@kwargs | select %file}

Rich type system

Crush does not have user defined types, but it does have a rich set of built in types, including

* Mutable and immutable lists of any type,
* Mutable and immutable dicts of any type,
* text,
* regular expressions,
* globs,
* files,
* booleans,
* integer numbers,
* structs, which contain any number of named fields of any type,
* tables, which are essentially a list where each element is the same type of struct,
* streams, which are like tables but can only be traversed once
* types,
* commands, which are either closures or built in commands.


Expressive language with static scoping rules and true closures:

DEMO CODE GOES HERE

Semi-lazy stream evaluation:

If you assign the output of the find command to a variable like so:

    crush> let all_the_files=(find /)

What will really be stored in the `all_the_files` variable is simply a stream. A small number
of lines of output will be eagerly evaluated, before the thread executing the find command
will start blocking. If the stream is consumed, for example by writing

    crush> echo $all_the_files

then all hell will break lose on your screen as tens of thousands of lines are printed to
your screen.

Another option would be to use the head command

    crush> val $all_the_files | head 1

Which will consume one line of output from the stream. This command can be re-executed until
the stream is empty.

SQL-like syntax for any type of input

Crush features many commands to operate om arbitrary streams of data using a SQL-like syntax.
These commands use field-specifiers like %foo to specify columns in the data stream that they
operate on:

echo $some_data | where {comp.eq $color green) | group %shoe_size | aggr green_shoes_of_size={count}

Unlike in SQL, these commands all operate on input streams, meaning they can be combined in
any order, and the input source can be file/http resources in a variety of formats or output of
commands like ps, find.

Modes

Just like in most command line shells, the default type of any data entered is text.
The following code will call the echo command with two arguments, the strings Hello and world.

crush> echo Hello world

Like most other scripting languages, crush uses the $-sigil to perform variable expansion:

    crush> let greeting="Hello, world!"
    crush> echo $greeting

But crush takes this a step longer by adding a variety of additional modes

Regular expression mode:

    crush> echo re{ab*c} 

The output of a regular expression mode literal is a regular expression object. This object can
be used for matching in any command that does matching, e.g. the where command.

Closures mode:

    crush> let greet={val Hello} 

