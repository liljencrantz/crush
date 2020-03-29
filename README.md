# Crush

Crush is an attempt to make a traditional command line shell
that is also a modern programming language. It has the features
one would expect from a convenient programming language like
a type system, closures and lexical scoping, but with a syntax
geared toward both batch and interactive shell usage.

## What features of a traditional shell does Crush retain?

The basic structure of the language.

How to invoke commands, pass arguments and set up pipelines are
unchanged, as is the central concept of a current working directory .
This means that trivial invocations, like `ls` or `find .. | count`
look the same, but under the hood they are quite different, and
nearly everything beyond that is different.

## What does Crush do so differently, then?

### Some examples

Let's start with a trivial command. Listing files in the current
directory, and checking how many files are in the current directory:

    crush> ls
    user         size modified                  type      file
    liljencrantz 2279 2020-03-07 13:00:33 +0100 file      ideas
    liljencrantz 4096 2019-11-22 21:56:30 +0100 directory target
    ...
    
    crush> ls | count
    14

This looks familiar. But apperances are deceiving. The  `ls` output
is actually a table of rows, and crush provides you with SQL-like
commands to sort, filter, aggregate and group lines.

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

Because crush output is a stream of rows with columns, actions like sorting
by an arbitrary column or filtering data based on arbitrary logical
expressions operating on these columns is easy, and because the components
used to do this are generic and reusable, you can trivially do the same
to data from any source, such as json files, http requests, etc.

### Operators for comparison, logical operations and arithmetical operations

Crush allows you to perform mathematical calulations on integer and floating
point numbers directly in the shell, using the same mathematical operators
used in almost any other programming language.

    crush> 5+6
    11
    crush> 1+2*3
    7

The only exception is that the `/` operator is used for constructing files and
paths, so division is done using the `//` operator

    crush> 4.2//3
    1.4000000000000001

Comparisons between values are done using `>`, `<`, `<=`, `>=`, `==` and `!=`,
just like in most languages. All comparisons between values of different types
are false.

    crush> 4 > 5
    false
    
The `and` and `or` operators are used to combine logical expressions:
    
    crush> false and true
    false
    
Crush also has operators related to patterns and matching.
`=~` and `!~` are used to check if a pattern matches an input:

    # The % character is the wildcard operator in globs
    crush> %.txt =~ foo.txt
    true
    # This is how you construct and match a regular expression
    crush> re"ab+c" =~ "abbbbbc"
    true

Regexps also support replacement using the `~` (replace once) and
`~~` (replace all) operators, which are trinary operators:

    crush> re"a+" ~ "baalaa" "a"
    balaa
    crush> re"a+" ~~ "baalaa" "a"
    bala


### Type system

As already mentioned, many crush commands operate on streams of tabular data. The
individual cells in this table stream can be any of a variety of types, including
strings, integers, floating point numbers, lists, binary data or another table
stream.

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

Variables must be declared (using the `:=` operator) before first usage.

    crush> some_number := 4      # The := operator declares a new variable
    crush> some_number * 5
    20

Once declared, a variable can be reassigned to using the `=` operator.

    crush> some_number = 6
    crush> some_number * 5
    30

Like in any sane programming language, variables can be of any type
supported by the type system. 

    crush> some_text := "hello"
    crush> some_text * some_number
    Error: Can not process arguments of specified type

### Subshells

Sometimes you want to use the output of one command as an *argument* to
another command. This is different from using the output as the *input*,
and is done using `()`:

    crush> echo (pwd)

### Closures

In crush, braces (`{}`) are used to create a closure. Assigning a closure
to a variable is how you create a function.

    crush> print_greeting := {echo "Hello"}
    crush> print_greeting
    Hello

Any named arguments passed when calling a closure and added to the local
scope of the invocation:

    crush> print_a := {echo a}
    crush> print_a a="Greetings"
    Greetings

For added type safety, you can declare what parameters a closure expects
at the start of a closure.

The following closure requires the caller to supply
the argument `a`, and allows the caller to specify the argument `b`, which must
by of type integer. If the caller does not specify it, it falls back to a
default value of 7.

    crush> print_things := {|a b: integer = 7|}

Additionally, the `@` operator can be used to create a list of all unnamed
arguments, and the `@@` operator can be used to create a list of all named
arguments not mentioned elsewhere in the parameter list.

    crush> print_everything := {|@unnamed @@named| echo "Named" named "Unnamed" unnamed}

The `@` and `@@` operators are also used during command invocation to perform the
mirrored operation. The following code creates an `lss` function that calls the `ls`
command and passes on any arguments to it, and pipes the output through the `select`
command to only show one column from the output.

    lss := {|@args @@kwargs| ls @args @@kwargs | select %file}

### Types

Work is being done to allow user defined types. Built in types
include

* lists of any type,
* dicts of any type,
* strings,
* regular expressions,
* globs,
* files,
* booleans,
* integer numbers,
* floating point numbers,
* structs, which contain any number of named fields of any type,
* tables, which are essentially a list where each element is the same type of struct,
* table streams, which are like tables but can only be traversed once
* binary data,
* binary streams, which are like binary data but can only be traversed once
* types, and
* commands, which are either closures or built in commands.

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

### Namespaces, members and methods

TODO

### The content of your current working directory lives in your namespace

TODO

### Semi-lazy stream evaluation:

If you assign the output of the find command to a variable like so:

    crush> all_the_files := (find /)

What will really be stored in the `all_the_files` variable is simply a stream. A small number
of lines of output will be eagerly evaluated, before the thread executing the find command
will start blocking. If the stream is consumed, for example by writing

    crush> all_the_files

then all hell will break loose on your screen as tens of thousands of lines are printed to
your screen.

Another option would be to pipe the output via the head command

    crush> all_the_files | head 1

Which will consume one line of output from the stream. This command can be re-executed until
the stream is empty.

### More SQL-like data stream operations

Crush features many commands to operate om arbitrary streams of data using a SQL-like syntax.
These commands use field-specifiers like ^foo to specify columns in the data stream that they
operate on:

    ps | where {user == "root") | group ^status | aggr proc_per_status={count}

Unlike in SQL, these commands all operate on input streams, meaning they can be combined in
any order, and the input source can be file/http resources in a variety of formats or output of
commands like ps, find.

### Globs

The `*` operator is used for multiplication, so Crush uses `%` as the wldcard operator. `?` is
still used for single character wildcards.

Wildcards are not automatically expanded, they are passed in to commands as glob objects,
and the command chooses what to match the glob against.

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

TODO

### Materilised data

TODO


