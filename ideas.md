# Extend argument declaration syntax

* Add the option of adding a documentation string for each argument.
* Add a syntax to allow documentation of the whole closure via short, long and example strings.
* Allow an optional separator between arguments so that you can put each argument on a separate line

```
$timeit := {
    |
        short="Execute a command many times and estimate the execution time."
        long="This function provides a simple way to time small bits of Crush code"
        example="timeit {files|sort size}"
        $it: $command "the command to time."
        $number: $integer "the number of runs in each repeat. If unspecified, timeit will repeat enough times for each batch to take roughly 0.4 seconds."
        $repeat: $integer = 5 "repeat count. The average speed in the fastest repeat will be returned."
    |
    ...
}
```

# XML serialization

Use struct:s with three members,

* `name`, the node name.
* `attr`, a `dict $string $string` containing the attributes of the node.
* `children`, a `list $any` containing text fragments (as strings) mixed with child nodes. 

# Changed variable declaration syntax

`let foo=bar`

More consistent with the regular crush syntax, but slightly more verbose.

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
Maybe unset should be an operator, so that we don't have to quote the variable name, which feels inconsistent
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
