# Crush

Crush is an attempt to make a traditional command line shell that is also a
modern programming language. It has the features one would expect from a modern
programming language like a type system, closures and lexical scoping, but with
a syntax geared toward both batch and interactive shell usage.

- [The overview](docs/overview.md) is a narrative tour of the features of Crush.
- [The language reference](docs/language_reference.md) documents the syntax and
  core language features (error handling, background jobs, warnings, pattern
  matching, and more) in depth.
- [The configuration documentation](docs/config.md) describes how to configure Crush.
- [The builtin reference](https://liljencrantz.github.io/crush/builtins.html) is a
  searchable page documenting every builtin command and namespace, generated directly
  from the running binary. (Rendered from [`docs/builtins.html`](docs/builtins.html),
  which GitHub shows as raw source rather than rendering.)

## Building and installing Crush

### OS X dependencies

* Install [Brew](https://brew.sh/).
* Install openssl `brew install openssl`
* Install protobuf `brew install protobuf`
* Install git `xcode-select --install`

### Ubuntu dependencies

* Update apt index `sudo apt update`
* Install dependencies `sudo apt install build-essential git curl pkg-config libssl-dev libdbus-1-dev libsystemd-dev protobuf-compiler-grpc`

### Install Rust

Install Rust

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

The installer adds `~/.cargo/bin` to your path in new shells. To use it in the
shell you installed it from, run

    . "$HOME/.cargo/env"

### Compile and install crush

clone this repository,

    git clone https://github.com/liljencrantz/crush.git
 
and run

    cd crush && cargo install --path .

and you should have a working binary to try out in `~/.cargo/bin`.
That directory should already be in your path, so just write `crush` to run
the shell.

Have fun!
