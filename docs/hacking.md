# Hacking on Crush

This document explains how to get started on hacking on Crush. Not hacking *with*
crush, but changing the shell itself. It is currently woefully incomplete.

## Writing your own commands

Commands are Rust functions that take an `CommandContext` as input and return a `CrushResult<()>`, e.g.

```rust
fn find(context: CommandContext) -> CrushResult<()> {
    ...
}
```

The execution context contain the input and output streams, program arguments, the
current scope, etc.

### Argument parsing

The correct way to parse the input argument is to write a signature struct using
the `signature` macro, for example:

```rust
#[signature(find, short="Recursively list files")]
pub struct Find {
    #[unnamed()]
    #[description("directories and files to list")]
    directory: Files,
    #[description("recurse into subdirectories")]
    #[default(true)]
    recursive: bool,
}
```

The signature macro will use the supplied information to generate efficient code
to parse your arguments as well as generate suitable output for the help command.

#### Available types for the argument parser

The available types for input arguments should be written down here, but this
document is a WIP, so they currently aren't. 

### Inserting the command into a namespace

In order to use the command, you will need to insert it somewhere in the Crush
namespace. This is done using an invocation like `Find::declare(env)?;`.

Creating a new namespace should also be documented here, but it isn't because
this document is a WIP. Check one of the existing namespaces under src/lib for
an example instead.

### Invoking the argument parser

The final bit of wiring needed is to actually *parse* your input argument during
command invocation, which is done using an invocation like

```rust
fn find(context: CommandContext) -> CrushResult<()> {
    let config: Find = Find::parse(context.arguments, &context.printer)?;
    ...
}
```

This will give you all your arguments, ready to be used.

## Reading input

WIP

## Writing output

WIP

# Crush values

WIP
