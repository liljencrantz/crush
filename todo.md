# Todo

* Allow commands to specify the type of input they expect for better/earlier validation
* Add system tests for binary stream handling
* Add package command to create a new namespace
* pbuf:from/to command that takes a protobuf definition and uses it to deserialize protobuf data
* avro:from/to command that deserializes avro data
* Support __str__ method for string rendering
* fix dynamic loading deadlocks
* tab completions for external commands
* Make IFS configurable for cmd command
* Better syntax highlighting for expression mode
* Figure out what to do about users/groups __getitem__
* Fix help messages in dbus connections
* Completion descriptions
* Support arbitrary filenames with hex escape codes in globs and other places
* Allow setting type for varargs in closures
* $binary_stream:pipe
* Namespace completions are sometimes wonky
* Namespace highlighting is sometimes wonky
* syntax highlighting does not understand closure signatures
* syntax highlighting does not understand closure parameters
* Regular file completion of strings
* Reserve `$global`. Possibly also `$true`, `$false` and a few more?
* Drop chmod/chown/touch and more methods from file. Make commands where appropriate.
