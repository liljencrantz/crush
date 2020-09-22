# Configuring Crush

## The Crush configuration file

When run in interactive mode, Crush will execute the file `$XDG_CONFIG_HOME/crush/config.crush`,
or `~/.config/crush/config.crush` if the above variable isn't defined. You can put any commands
you want to run before startup there.

## Crush state

The namespace `crush` contains all the Crush state, including all the aspects of Crush that
can be configured.

### Configuring the Crush prompt

To configure the Crush prompt, call the `crush:prompt` command, and pass in a closure that
returns a string, for example:

```shell script
crush:prompt {"{user}@{host} {wd}# ":format wd=(pwd) user=(user:me:name) host=(host:name) }
```

### Locale

The `crush:locale` namespace contains three methods:

- `crush:locale:list` lists all locales supported by your operating system,
- `crush:locale:set` updates the current locale, and
- `crush:locale:get` returns the current locale.

Currently, the only way that the locale influences how Crush operates is where
underscores are inserted into integer numbers to simplify reading of large numbers.

