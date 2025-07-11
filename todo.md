# Todo

* Rewrite all tests for new syntax
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
* Better syntax highlighting for expression mode
* Move yaml builtin to use saphyr
* Figure out what to do about users/groups __getitem__
* Fix help messages in grpc connections
* Fix help messages in dbus connections
* Completion descriptions
* Arbitrary filenames via hex escape codes
* Allow setting type for varargs in closures
* When a closure specifies a list-type parameter, make the argument parser accept multiple instances of the specialization that are then turned into a list.
* The Files type for signatures has different valid values when serving as input and output. This makes the documentation misleading. Split in two?
* base64 lib
* $binary_stream:pipe
