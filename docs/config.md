# Configuring Crush

## The Crush configuration file

When run in interactive mode, Crush executes up to two config files on startup, in
this order:

1. `/etc/config.crush`, a system-wide config file, if it exists.
2. `$XDG_CONFIG_HOME/crush/config.crush`, or `~/.config/crush/config.crush` if that
   variable isn't set, if it exists.

Both are optional, and you can put any commands you want to run before startup in
either one. Since the user file runs second, anything it sets (the prompt, syntax
highlighting colors, etc.) overrides what the system file set up. Config files are
only read in interactive mode — running `crush somefile.crush` does not read either
one.

## Crush state

The namespace `crush` contains all the Crush state, including all the aspects of Crush that
can be configured.

### Configuring the Crush prompt

To configure the Crush prompt, call the `crush:prompt:set` command, and pass in a closure that
returns a string, for example:

```shell script
crush:prompt:set {"{user}@{host} {wd}# ":format wd=(pwd) user=(user:me:name) host=(host:name) }
```

If you want your Crush prompt to be colorful, the `term` namespace contains useful
constants containing ANSI color codes for altering the look of your prompt.
A slightly more colorful version of the above prompt would be:

```shell script
crush:prompt:set {
    "{green}{user}{normal}@{host} {green}{wd}{normal}# ":format wd=$(pwd) \
        user=$(user:me:name) host=$(host:name) \
        green=$(term:green) normal=$(term:normal)
}
```

### Configuring the Crush title message

To configure the Crush prompt, call the `crush:title:set` command, and pass in a closure that
returns a string, for example:

```shell script
crush:title:set {
      "{user}@{host} {wd}":format wd=(pwd) user=(user:me:name) host=(host:name) 
}
```

### Configuring syntax highlighting

The dict `crush:highlight` allows you to customize the syntax highlighting of
Crush code in the interactive Crush prompt. Assign ANSI color codes
to the various token types of Crush to make your terminal more closely
resemble a Christmas tree:

| Name              | Description                                               |
|-------------------|-------------------------------------------------------------|
| `command`         | Commands                                                   |
| `comment`         | Comments                                                   |
| `error`           | Error messages                                             |
| `file_literal`    | File literals, like `'Cargo.toml'`                         |
| `glob_literal`    | Glob literals like `*.txt`                                 |
| `identifier`      | Variables and members, like `$global`                      |
| `keyword`         | Reserved words like `continue` and `break`                 |
| `numeric_literal` | Integer and floating point literals, such as `6`           |
| `operator`        | All the different Crush operators, such as `neg` and `+`   |
| `regex_literal`   | Regex literal like `^(a*)`                                 |
| `string_literal`  | String literals, like `"Burrow"`                            |
| `warning`         | Warning messages                                            |

The `term` namespace contains useful constants containing ANSI color codes.
A configuration example:

```shell script
crush:highlight[file_literal] = $(term:cyan)
crush:highlight[string_literal] = $(term:yellow)
crush:highlight[numeric_literal] = $(term:magenta)
```

### Locale

The `crush:locale` namespace contains three methods:

- `crush:locale:list` lists all locales supported by your operating system,
- `crush:locale:set` updates the current locale, and
- `crush:locale:get` returns the current locale.

The locale currently affects Crush in two ways:

- It controls the grouping pattern used when printing large integers — where the
  digits get split up. The separator itself is always `_`, never whatever character
  the locale would normally use, so the output stays safe to paste back in as a
  literal.
- If no temperature unit has been set explicitly (there is currently no command to
  do this — see "Not yet configurable" below), it picks a default based on your
  locale's country: the US and a handful of other Fahrenheit-using countries get
  Fahrenheit, every other recognized country gets Celsius, and an unset or
  unrecognized locale falls back to Kelvin.

### Byte unit formatting

The `crush:byte_unit` namespace controls how table columns containing byte sizes
(e.g. file sizes) are displayed:

- `crush:byte_unit:list` lists the available units: `binary` (powers of 1024, e.g.
  `KiB`/`MiB`), `decimal` (powers of 1000, e.g. `kB`/`MB`), and `raw` (a plain
  grouped integer, with no unit suffix).
- `crush:byte_unit:set` changes the current unit. The default is `binary`.
- `crush:byte_unit:get` returns the current unit.

```shell script
crush:byte_unit:set decimal
```

### Warning limit

Commands that continue past a partial failure instead of aborting (e.g. one bad row
out of a stream — see `each`/`where`/`group`/`files`) report it as a warning;
`crush:warnings` lists the most recent ones. `crush:warning_limit` controls how many
are kept before the oldest is evicted:

- `crush:warning_limit:set` changes the limit. The default is 100.
- `crush:warning_limit:get` returns the current limit.

### Environment variables

The `crush:env` namespace reads and writes the process's own OS environment
variables — the same environment external commands (and Crush itself, e.g. for
`PATH` lookups) see:

```shell script
crush:env["MY_VAR"] = "some value"
echo $(crush:env["MY_VAR"])
crush:env:unset "MY_VAR"
crush:env:list
```

### Not yet configurable

A few display-formatting knobs exist internally — the number of digits used when
printing a float, a percentage, or a temperature, and the temperature unit itself
(Celsius/Fahrenheit/Kelvin, see "Locale" above) — but as of this writing there is no
command wired up to change them. They're fixed at their defaults: 4 digits for
floats, 2 for percentages, 1 for temperatures.
