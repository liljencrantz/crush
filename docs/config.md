# Configuring Crush

## The Crush configuration file

When run in interactive mode, Crush will execute the file `$XDG_CONFIG_HOME/crush/config.crush`,
or `~/.config/crush/config.crush` if the above variable isn't defined. You can put any commands
you want to run before startup there.

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
crush:title:set {"{user}@{host} {wd}":format wd=(pwd) user=(user:me:name) host=(host:name) }
```

### Configuring syntax highlighting

The dict `crush:highlight` allows you to customize the syntax highlighting of
Crush code in the interactive Crush prompt. Assign ANSI color codes
to the various token types of Crush to make your terminal more closely
resemble a Christmas tree:

| Name              | Description                                              |
|-------------------|----------------------------------------------------------|
| `command`         | Commands                                                 |
| `comment`         | Comments                                                 |
| `file_literal`    | File literals, like `'Cargo.toml'`                       |
| `glob_literal`    | Glob literals like `*.txt`                               |
| `keyword`         | Reserved words like `continue` and `break`               |
| `label`           | Variables and members, like `$global`                    |
| `numeric_literal` | Integer and floating point literals, such as `6`         |
| `operator`        | All the different Crush operators, such as `neg` and `+` |
| `regex_literal`   | Regex literal like `^(a*)`                               |
| `string_literal`  | String literals, like `"Burrow"`                         |

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

Currently, the only way that the locale influences how Crush operates is where
underscores are inserted into integer numbers to simplify reading of large numbers.

