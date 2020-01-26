Crush

This is an experimental command line shell, intended both for interactive and non-interactive use.
The goal of this shell is to figure out of the following changes to a regular unix-style shell make
for a a powerful environment:

Typed input/output

The input and output of pipe lines aren't simply streams of bytes, they can be any type in a rich type
system. The most common input/output type is a stream of rows, where each row consists of columns, such
as from the ls command:

    crush> ls 

    bla bla bla

But other commands output a single value, such as pwd, which outputs the current working directory
as the file type.

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


Expressive language with static scoping rules and true closures

DEMO CODE GOES HERE

Semi-lazy stream evaluation

If you assign the output of the find command to a variable like so:

    crush> let all_the_files={find /}

What will really be stored in the all_The_files variable is simply a stream. A small number
of lines of output will be eagerly evaluated, before the thread executing the find command
will start blocking. If the stream is consumed, for example by writing

    crush> echo $all_the_files

then all hell will break lose on your screen as tens of thousands of lines are printed to
your screen.

Another option would be to use the take command

    crush> take all_the_files

Which will consume one line of output from the stream. This command can be re-executed until
the stream is empty.

SQL-like syntax for any type of input

Crush features many commands to operate om arbitrary streams of data using a SQL-like syntax.
These commands use field-specifiers like %foo to specify columns in the data stream that they
operate on:

echo $some_data | where %color == green | group %shoe_size | aggr green_shoes_of_size=`{count}

Unlike in SQL, these commands all operate on input streams, meaning they can be combined in
any order, and the input source can be file/http resources in a variety of formats or putput of
commands like ps, find.

Modes

Just like in most command line shells, the default type of any data entered is text.
The following code will call the echo command with two arguments, the strings Hello and world.

crush> echo Hello world

Like most other scripting languages, crush uses the $-sigil to perform variable expansion:

    crush> let greeting = "Hello, world!"
    crush> echo $greeting

But crush takes this a step longer by adding a variety of additional modes

Regular expression mode:

    crush> echo re{ab*c} 

The output of a regular expression mode literal is a regular expression object. This object can
be used for matching in any command that does matching, e.g. the where command.

Expression mode:

    crush> echo expr{1+1} 

In expression mode, crush functions a lot more like a normal scripting language. The $-sigil is
not required, strings must be escaped, and crush has a large number of additional operators like
+ - * / that perform the expected operation.
