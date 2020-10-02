# Crush

Crush is an attempt to make a traditional command line shell that is also a
modern programming language. It has the features one would expect from a modern
programming language like a type system, closures and lexical scoping, but with
a syntax geared toward both batch and interactive shell usage.

- [The overview](docs/overview.md) gives a detailed overview of the
  features of Crush. 
- [The syntax documentation](docs/syntax.md) contains more a detailed description
  of the Crush syntax than what fits into the overview document. 
- [The configuration documentation](docs/config.md) describes how to configure Crush.
- [The hacking documentation](docs/hacking.md) will eventually give you enough
  information about the inner workings of Crush to start hacking yourself.

## Building and installing Crush

Crush should work on any modern Unix system. Install rust,
 
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

clone this repository,

    git clone https://github.com/liljencrantz/crush.git
 
and run

    cd crush; cargo build

and you should have a working binary to try out.

Have fun!
