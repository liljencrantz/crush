# More useful and expressive and bg and fg functionality

```
# Make the job handles returned by the bg command opaque, such that printing them
# does not block. This means the following would now work as expected:

files --recurse / | count | bg

# In addition, this new handle should have a `wait` method that waits for the command
# to exit and returns the value:

$handle := $(files --recurse / | count | bg)
$handle:wait

# Then add a syntactic sugar for the bg command using the `&` operator.
# `&` should basically be equivalent to `| bg;`, i.e.  
# these two become completely equivalen:

files --recurse / | count | bg
files --recurse / | count &

# The `bg` command keeps a stack of all the commands put into the background,
# so that instead of writing 

$handle := $(files --recurse / | count &)
$handle:wait

# you can alternatively write

files --recurse / | count &
fg

# The latter is simpler and more useful in interactive situations, 
# while the former is clearer in complex situations and more suitable for scripting.
```

# Pluggable tab completion framework

Individual commands should be able to provide tags (possibly mime tags?)
that point to specific completions, `hostname`, `uri`, `git/repo`, or `git/branch`.
Completions for these tags can then be reused between different commands, and the 
user can install additional completion sources.

Potentially, the whole tab completion framework could also be shared between
different shells.

(Idea comes from Marcus Vesterlund)

# binary pipe method
```shell
$# Create a pipe
$pipe := $(binary_stream:pipe)

# Create a job that writes base64 encoded data to the pipe
$_1 := $(base64:to | pipe:write | bg)
# Create a second job that reads from the pipe and sums all the integers and put this job in the background
$sum_job_handle := $(pipe:read | sha1 | bg)
# Close the pipe so that the second job can finish
pipe:close
# Put the sum job in the foreground
sum_job_handle | fg
```

# More help topics

We could add crush help topics accessible via topic strings like `help topic=closures`.
These topics could be small file snippets that we then compile into the various bigger files in the
docs subdirectory. They would be discoverable via tab completion.

# XML serialization

Use struct:s with three members,

* `name`, the node name.
* `attr`, a `dict $string $string` containing the attributes of the node.
* `children`, a `list $any` containing text fragments (as strings) mixed with child nodes.

# Changed variable declaration and assignment syntax

```
# Declare new variable
let foo=bar
# Assign new value to existing variable
set foo=bar
```

More consistent with the regular crush syntax, but more verbose.

# Validation

All commands declare valid input and output types.
Input types can be partial, e.g. any iterator or any iterator with some restrictions.
Checks are performed to validate consistency.
Syntax for not having to duplicate output type.
Track location of arguments through signature macro parsing

# New and updated builtins

A simple command for replacing a regex in every line of a file. Implement it in crush, using built in commands.
Extra columns for ps: tty, current CPU usage.
A grep-command.
Simple column renaming in select, e.g. 'ps|select time=cpu'
xml:from/to using html5ever under the hood
html:from/to using html5ever under the hood
Add hex and base64 en/decoding methods to the binary/string types
Add utf-8 and other character encoding methods to the binary/string types
Maybe unset should only be able to delete members of the current scope
sticky bits support for chmod
recursive chmod
recursive chown
watch command
Either stop copying Command instances, or make them pointers to static data.
Syntax highlighting (with more accurate parenthesis matching)

Every command should have a command id within that job, e.g. 5:2
Every thread should have a thread id within that command, e.g. 5:2:3
Make command closing work on file literals
Make suggestions use completion engine instead of history
Handle ^Z to put jobs into background. How?
Handle ^C to cancel jobs. How?

# Tab completion missing feature list

* Support for enabling and disabling completion error printing
* fall back to "stupid" completion if parsing the AST fails
* Completions for wildcards
* Complete with prior arguments from same command from history
* Add command specific completions for dbus
* Add command specific completions for cd to filter only directories
* Add previous output type of previous command in pipeline if known to parsed state
* Add command specific completions for methods
* GetItem and SetItem completions
