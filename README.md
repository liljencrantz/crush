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

### OS X dependencies

* Install [Brew](https://brew.sh/).
* Install openssl `brew install openssl`
* Install protobuf `brew install protobuf`
* Install git `xcode-select --install`

### Ubuntu dependencies

* Update apt index `apt update`
* Install dependencies `apt install build-essential git curl pkg-config libssl-dev libdbus-1-dev libsystemd-dev`

### Install Rust

Install Rust

    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

Add it to your path

    PATH=$PATH:$HOME/.cargo/bin

### Compile and install crush

clone this repository,

    git clone https://github.com/liljencrantz/crush.git
 
and run

    cd crush && cargo build --release && cargo install --path .

and you should have a working binary to try out in `~/.cargo/bin`.
That directory should already be in your path, so just write `crush` to run
the shell.

Have fun!
