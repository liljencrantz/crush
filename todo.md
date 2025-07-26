# Todo

* Rewrite all tests for new syntax
* Add command field to printer
* Allow commands to specify the type of input they expect for better/earlier validation
* Write a command that extracts all help into html
* Add system tests for binary stream handling
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
* Better syntax highlighting for expression mode
* Move yaml builtin to use saphyr
* Figure out what to do about users/groups __getitem__
* Fix help messages in grpc connections
* Fix help messages in dbus connections
* Completion descriptions
* Support arbitrary filenames with hex escape codes in globs and other places
* Allow setting type for varargs in closures
* $binary_stream:pipe
* Namespace completions are sometimes wonky
* Namespace highlighting is sometimes wonky
* syntax highlighting does not understand using types as commands
* syntax highlighting does not understand closure signatures
* syntax highlighting does not understand closure parameters
