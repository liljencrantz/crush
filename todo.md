# Todo

* Rewrite all tests for new syntax
* Add some way to extract the definition from a closure, e.g. `$my_function:definition`
* Add command field to printer
* Allow commands to specify the type of input they expect for better/earlier validation
* Write a command that extracts all help into html
* Tab completion of globs
* Add system tests for binary stream handling
* Data enums + pattern matching syntax
* Add package command to create a new namespace
* pbuf:from/to command that takes a protobuf definition and uses it to deserialize protobuf data
* avro:from/to command that deserializes avro data
* Support __str__ method for string rendering
* fix dynamic loading deadlocks
* tab completions for external commands
* More shell-like syntax for background jobs
* Make IFS configurable for cmd command
* Add signal handlers to fix ^C and ^Z during regular execution
* Dict literals in expression mode ({key: value})
* {|$foo: $glob or $string or $regex|} to specify that an argument is one of several types
* Better syntax highlighting for expression mode
* Better syntax highlighting of possible errors
* Operator ordering is unintuitve sometimes e.g. `$s == $s:global`. At least throw an error instead of doing the wrong thing?
* Move yaml builtin to use saphyr
* Figure out what to do about users/groups __getitem__
