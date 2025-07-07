# Definitions

This document contains the definitions Crush uses for various words. 

## Commands, closures, builtins

* A Builtin command is a command that is implemented as a part of the crush binary itself.
* A Closure command is a command that is defined in the Crush language and therefore implemented as a combination of other commands.
* A Block command is more lightweight than a closure command. It does not have a parameter list or a name. They are mainly used to provide callbacks for commands such as `for`, `where` or `if`.
* An external command is a command that is not part of Crush itself, such as `git`, `emacs` or `ssh`. External commands only support string arguments and their pipes always use binary stream data. 

## Signatures, parameters and arguments 

In crush, the word signature means the set of all parameters a given command accepts, including type restrictions, and default values. For example, the `http` command has a signature containing 5 parameters, including `uri` and `timeout`.

A signature is made up of parameters, which are the definitions of the individual values a command accepts. The `http` command has a parameter named `timeout`. It has the type of duration and a default value of 5 seconds.

An argument is a value passed in to a parameter for a command. If we write `http "http://example.com" timeout=$(duration:of seconds=30)`, we are passing in the argument of 30 seconds for the `timeout` parameter.

An argument can be named or unnamed. These are just different styles of passing in values to a command. For example, we can set the uri for the `http` command named or unnamed:

```
# Completely equivalent
http "http://example.com" 
http uri="http://example.com" 
```
