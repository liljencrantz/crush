# Crush syntax

## Commands

The structure of a Crush command is a space separated list.
The first element of the list is the command, the remaining
elements are the arguments:

```shell script
echo 5
git commit message="This commit is amazing"
```

### Named and unnamed arguments

arguments to commands can be passed in named or unnamed:

```shell script
# These calls are equivalent
http "https://example.com/"
http uri="https://example.com/"
```

Argument mapping works as follows:

* First, all named arguments are assigned.
* Then, each unnamed argument is assigned to the first argument
  that is currently not assigned a value.

One can also optionally specify that either stray named or stray unnamed
arguments should be collected into a dict or list.

## Jobs and pipelines

Commands do not only accept arguments. They also accept a single value as it's
input and produce a single value as output. The input and output of a command
is passed down via a so called pipeline:

```shell script
host:procs | sort cpu
```

Many commands consume and produce table streams as input and output. These commands
run concurrently, so that the whole result need not be produced before the next step
in the pipeline begins work.

The separation of concerns between arguments and input to command is that command
arguments configure how a the data should be processed, the input is the data
to process and the output is where the processed data ends up.

## String literals, variables, file literals

A character sequence enclosed within double quotes become a string literal value,
e.g. `"hello"`. Unquoted character sequences containing only A-Z, a-z, 0-9 and _
are also strings, e.g. `user` or `hat`.

Unquoted character sequences containing a the letters `%` or `?` are a glob,
which is an object that can be used for matching strings.

A character sequence enclosed within single quotes become a file literal, e.g.
`'Cargo.yaml'`, i.e. a value of the type `file`. An unquoted character sequence
that contains a dot (`.`) or a slash ('/'), or begins with a tilde (`~`) is also
interpreted as a file literal, e.g. `Cargo.yaml` or `~/.ssh`.

A character sequence starting with a caret (`$`) is intepreted as a variable
lookup. The first sequence in a command (i.e. the command name) is interpreted
as a variable lookup even without the leading `$`. Commands live in
the same namespace as all other variables.

## Operators

Crush features a number of operators to enable users to write mathematical
expressions and a few other operators that are a lot easier to read the code.

These are presented below in order of precedence.

| operator                    | Example                               | Description                                           |
|-----------------------------|---------------------------------------|-------------------------------------------------------|
| `:=` `=`                    | `$foo := 7`                           | Declare a new variable, reassign an existing variable |
| `and` `or`                  | `foo:is_file and foo:stat:len > 4096` | Logical operators                                     |
| `>` `>=` `<` `<=` `==` `!=` | `foo > 5`                             | Compare two values to each other                      |
| `+` `-`                     | `1+1`                                 | Addition and subtraction                              |
| `*` `//`                    | `5*5`                                 | Multiplication and division                           |
| `typeof`                    | `typeof foo`                          | The type of a value                                   |
| `neg` `not`                 | `neg 5`                               | Numeric and logical negation                          |

## Command substitutions

It is often useful to use the output of a crush command as an argument to a different
command. To do this, simply put the command within a subshell `$()`:

```shell script
"Hello, {name}":format name=$(user:me:name)
```

if the command that you run in a substitution returns a stream, the outer command will
be run in parallel with the substitution and may in fact finish running first.

This example will create a command that locates all the files in your computer, and assigns
the output stream to a variable. The `files` command in this example will block because there
is nothing reading its output.

```shell script
$all_the_files := $(files --recurse /)
$all_the_files | head 1
```

## Namespaces

Unlike other shells, Crush relies heavily on namespaces to separate different commands
and avoid name clashes. Crush uses `:` as the namespace operator as well as the member
access operator. (This is because the content of a namespace is simply its members)

A few of the namespaces in Crush are:

* the `crush` namespace, which contains runtime information about the current shell
  as well as methods and variables that allow you to reconfigure it,
* the `user` namespace which contains information about all the users of this system,
* the `fd` namespace, which contains information about open file descriptors, e.g. all
  open network sockets, unix sockets and open files, and
* the `host` namespace, which contains information about the current host, including
  host name, CPU status, memory usage and operating system.
