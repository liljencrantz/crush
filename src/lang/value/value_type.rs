use crate::builtins::types;
use crate::lang::command::OutputType::Known;
/// All the different types a value can have.
use crate::lang::command::{Command, OutputType};
use crate::lang::errors::{CrushResult, command_error, error};
use crate::lang::help::Help;
use crate::lang::{data::table::ColumnType, value::Value};
use crate::util::glob::Glob;
use itertools::Itertools;
use ordered_map::OrderedMap;
use regex::Regex;
use std::fmt::{Display, Formatter};
use std::sync::OnceLock;
use const_format::formatcp;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum ValueType {
    String,
    Integer,
    Time,
    Duration,
    Glob,
    Regex,
    Command,
    File,
    TableInputStream(Vec<ColumnType>),
    TableOutputStream(Vec<ColumnType>),
    Table(Vec<ColumnType>),
    Struct,
    List(Box<ValueType>),
    Dict(Box<ValueType>, Box<ValueType>),
    Scope,
    Bool,
    Float,
    Empty,
    Any,
    BinaryInputStream,
    Binary,
    Type,
    OneOf(Vec<ValueType>),
}

pub fn empty_methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| OrderedMap::new())
}

impl ValueType {
    pub fn table_input_stream(columns: &[ColumnType]) -> ValueType {
        ValueType::TableInputStream(columns.to_vec())
    }

    pub fn output_type(&self) -> OutputType {
        Known(self.clone())
    }

    pub fn one_of(options: Vec<ValueType>) -> ValueType {
        let mut res = Vec::new();
        for vt in options {
            match vt {
                ValueType::Any => return ValueType::Any,
                ValueType::OneOf(mut vt) => res.append(&mut vt),
                _ => res.push(vt),
            }
        }
        ValueType::Any
    }

    pub fn fields(&self) -> &OrderedMap<String, Command> {
        match self {
            ValueType::List(_) => &types::list::methods(),
            ValueType::Dict(_, _) => &types::dict::methods(),
            ValueType::String => &types::string::methods(),
            ValueType::File => &types::file::methods(),
            ValueType::Regex => &types::re::methods(),
            ValueType::Glob => &types::glob::methods(),
            ValueType::Integer => &types::integer::methods(),
            ValueType::Float => &types::float::methods(),
            ValueType::Duration => &types::duration::methods(),
            ValueType::Time => &types::time::methods(),
            ValueType::Table(_) => &types::table::methods(),
            ValueType::TableInputStream(_) => &types::table_input_stream::methods(),
            ValueType::TableOutputStream(_) => &types::table_output_stream::methods(),
            ValueType::Binary => &types::binary::methods(),
            ValueType::Scope => &types::scope::methods(),
            ValueType::Struct => &types::r#struct::methods(),
            ValueType::OneOf(_) => &types::one_of::methods(),
            _ => empty_methods(),
        }
    }

    pub fn is(&self, value: &Value) -> bool {
        self.is_compatible_with(&value.value_type())
    }

    pub fn is_compatible_with(&self, pattern: &ValueType) -> bool {
        match self {
            ValueType::Any => true,
            ValueType::OneOf(types) => types.iter().any(|t| t.is_compatible_with(pattern)),
            _ => self == pattern,
        }
    }

    pub fn materialize(&self) -> CrushResult<ValueType> {
        Ok(match self {
            ValueType::String
            | ValueType::Integer
            | ValueType::Time
            | ValueType::Duration
            | ValueType::Glob
            | ValueType::Regex
            | ValueType::Command
            | ValueType::File
            | ValueType::Scope
            | ValueType::Float
            | ValueType::Empty
            | ValueType::Any
            | ValueType::Binary
            | ValueType::Type
            | ValueType::Struct
            | ValueType::Bool => self.clone(),
            ValueType::BinaryInputStream => ValueType::Binary,
            ValueType::TableInputStream(o) => ValueType::Table(ColumnType::materialize(o)?),
            ValueType::TableOutputStream(_) => {
                return command_error("Can't materialize `$table_output_stream`");
            }
            ValueType::Table(r) => ValueType::Table(ColumnType::materialize(r)?),
            ValueType::List(l) => ValueType::List(Box::from(l.materialize()?)),
            ValueType::Dict(k, v) => {
                ValueType::Dict(Box::from(k.materialize()?), Box::from(v.materialize()?))
            }

            ValueType::OneOf(types) => ValueType::OneOf(
                types
                    .iter()
                    .map(|t| t.materialize())
                    .collect::<CrushResult<Vec<_>>>()?,
            ),
        })
    }

    pub fn is_hashable(&self) -> bool {
        match self {
            ValueType::Scope
            | ValueType::List(_)
            | ValueType::Dict(_, _)
            | ValueType::Command
            | ValueType::BinaryInputStream
            | ValueType::TableInputStream(_)
            | ValueType::Struct
            | ValueType::Table(_) => false,
            ValueType::OneOf(types) => types.iter().all(|t| t.is_hashable()),
            _ => true,
        }
    }

    pub fn is_comparable(&self) -> bool {
        self.is_hashable()
    }

    pub fn parse(&self, s: &str) -> CrushResult<Value> {
        match self {
            ValueType::String => Ok(Value::from(s)),
            ValueType::Integer => match s.parse::<i128>() {
                Ok(n) => Ok(Value::Integer(n)),
                Err(e) => error(e.to_string().as_str()),
            },
            ValueType::Glob => Ok(Value::Glob(Glob::new(s))),
            ValueType::Regex => Ok(Value::Regex(s.to_string(), Regex::new(s)?)),
            ValueType::File => Ok(Value::from(s)),
            ValueType::Float => Ok(Value::Float(s.parse::<f64>()?)),
            ValueType::Bool => Ok(Value::Bool(s.parse::<bool>()?)),
            _ => error(format!("Can't parse string into value of type `{}`", self)),
        }
    }

    pub fn is_parametrized(&self) -> bool {
        match self {
            ValueType::List(_)
            | ValueType::Dict(_, _)
            | ValueType::TableOutputStream(_)
            | ValueType::TableInputStream(_)
            | ValueType::Table(_)
            | ValueType::OneOf(_) => true,
            _ => false,
        }
    }

    pub fn subfmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if self.is_parametrized() {
            f.write_str("$(")?;
        } else {
            f.write_str("$")?;
        }
        self.fmt(f)?;
        if self.is_parametrized() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl Help for ValueType {
    fn signature(&self) -> String {
        format!("type {}", self)
    }

    fn short_help(&self) -> String {
        match self {
            ValueType::String => {
                "Textual data, stored as an immutable sequence of unicode code points."
            }
            ValueType::Integer => "A numeric type representing an integer number.",
            ValueType::Time => "A point in time with nanosecond precision.",
            ValueType::Duration => "A difference between two points in time.",
            ValueType::Glob => "A pattern containing wildcards.",
            ValueType::Regex => "A regular expression is an advanced pattern that can be used for matching and replacing text.",
            ValueType::Command => "A piece fo code that can be called.",
            ValueType::File => "Any type of file.",
            ValueType::TableInputStream(_) => "An input stream of table rows.",
            ValueType::TableOutputStream(_) => "An output stream of table rows.",
            ValueType::Table(_) => "A table of rows.",
            ValueType::Struct => "A mapping from name to value.",
            ValueType::List(_) => "A mutable list of items, usually of the same type.",
            ValueType::Dict(_, _) => "A mutable mapping from one set of values to another.",
            ValueType::Scope => "A scope in the Crush namespace.",
            ValueType::Bool => "True or false.",
            ValueType::Float => {
                "A numeric type representing any number with floating point precision."
            }
            ValueType::Empty => "Nothing.",
            ValueType::Any => "Any type.",
            ValueType::BinaryInputStream => "A stream of binary data.",
            ValueType::Binary => "Binary data.",
            ValueType::Type => "A type.",
            ValueType::OneOf(types) => {
                return format!("One of {}", types.iter().map(|t| t.to_string()).join(", "));
            }
        }
        .to_string()
    }

    fn long_help(&self) -> Option<String> {
        let mut lines = vec![match self {
            ValueType::String =>
                    "A string literal is written between double quotes, e.g. `\"hello world\"`. A
string is a sequence of legal unicode characters -- nothing else can be represented, and
every crush string is guaranteed to be valid text.

Strings are immutable -- every method that looks like it modifies a string (`upper`,
`replace`, `trim`, ...) returns a new string rather than changing the receiver.

`file` and `binary` are the two other types that hold sequences of data rather than a
single scalar value, and it's worth being precise about how they differ from `string`
and from each other. A `file` represents an operating system path: a sequence of bytes
that is a legal file name, a system-dependent notion of legality that usually allows byte
sequences that aren't legal unicode (for example, a Latin-1-encoded name on a
system whose text encoding is UTF-8). A `binary` is simply a sequence of bytes with no
legality constraint at all -- the type to reach for when the data isn't necessarily text
or a path. Because \"legal file name\" and \"legal unicode text\" are overlapping but
different constraints, `string` and `file` have to be separate types: some strings can't
be legal file names (most commonly one containing a zero byte -- many operating systems
represent a path internally as a zero-terminated byte sequence, so a zero byte can never
be part of one), and some legal file names can't be represented as a string at all.

For convenience, builtin commands that expect a `file` argument also accept a `string`,
which crush converts automatically. That conversion doesn't validate the result up
front, though -- passing a string containing a zero byte is accepted silently, and only
fails once the resulting file value is actually used against the filesystem. Going the
other way, a `file` whose bytes aren't valid unicode can't be losslessly converted to a
`string` either; rather than erroring, crush falls back to displaying it as the
placeholder text `<invalid filename>`.",

            ValueType::Integer =>
                    formatcp!("An integer literal is a bare number, e.g. `5` or `-3`. Underscores may
be used as digit separators to make large numbers easier to read, e.g. `1_000_000`.

A Crush integer uses signed 128 bit precision. This means that the highest number that
can be represented is {}, and the lowest is {}.

Integers are immutable values. The most similar type is `float`, used for numbers that
need a fractional part; mixing an integer and a float in an arithmetic expression
promotes the result to a float.", i128::MAX, i128::MIN),

            ValueType::Float =>
                    "A float literal is a bare number containing a decimal point, e.g. `5.0` or
`-3.25`.

A Crush float is a IEEE 754 64-bit (double precision) floating point number. Floats are
immutable values. The most similar type is `integer`; mixing the two in an arithmetic
expression promotes the result to a float.",

            ValueType::Bool => "A boolean value is one of the two literals `$true` or `$false` --
there is no other way to construct one. Booleans are immutable, and are the result type
of every comparison (`==`, `<`, ...) and logical (`and`, `or`) operator.",

            ValueType::Duration =>
                    "To create your own duration objects, use the `duration:of` method, for example

    duration:of seconds=10

A duration instance has nanosecond precision. It is represented internally as two 64 bit numbers,
one for the number of seconds, and one for the nanosecond remainder.

durations are signed, i.e. they can be used to denote a negative span of time. Durations
are immutable values. The most similar type is `time`: subtracting one `time` from
another produces a `duration`, and a `duration` can be added to or subtracted from a
`time`.",

            ValueType::Time =>
                    "To get the current time, use `time:now`. To parse a time from text, use
`time:parse`.

All time instances use the local time zone.

A time instance has nanosecond precision. It is represented internally as two 64 bit numbers, one
for the number of seconds since the Unix epoc, and one for the nanosecond remainder.

Times are immutable values -- arithmetic methods like adding a `duration` return a new
time rather than changing the receiver. The most similar type is `duration`, used to
represent the difference between two times.",

            ValueType::Glob => "Globs are usually created by writing an unescaped string containing
a wildcard character (`*` or `?`), like `files *.toml`.

If you want to construct a new glob from a string, use the `glob:new` command, e.g.
`glob:new \"*.txt\"`.

Globs are immutable. The most similar types are `re`, which supports much richer
patterns at the cost of more complex syntax, and `string`, which globs otherwise
resemble but never match by wildcard -- only a real glob value does.",

            ValueType::Regex => "Regular expressions are usually created by writing using regexp
literal syntax, e.g. `files ^(^...$)`.

If you want to construct a new glob from a string, use the `re:new` command, e.g.
`re:new \"[a-z]*\\.txt\"`.

Regular expressions are immutable. The most similar type is `glob`, which supports only
simple wildcard patterns but with much simpler syntax.",

            ValueType::Command =>
                    "The most common way to create a command is a closure literal, e.g. `{echo hello}`,
or with named parameters, `{|$x| echo $x}`. Builtin commands are themselves command
values and can be captured into a variable the same way, e.g. `$e := $echo`.

A command value is immutable -- calling it runs its body, but that doesn't change the
value itself. Reassigning the variable that holds a command is a separate operation from
mutating the command.",

            ValueType::File =>
                    "A file value is usually written as a bareword or single-quoted path, e.g.
`./Cargo.toml` or `'my file.txt'` -- crush recognizes these as files rather than plain
strings based on their syntax. You can also convert an existing string explicitly, e.g.
`convert $file \"./Cargo.toml\"`.

A file value simply names a path; the value itself is immutable, though of course the
file it points at on disk can change, be created, or be removed out from under it via
methods like `remove` or commands like `fs:mkdir`. See `help string` for exactly how
`file` differs from `string` and `binary`, the other two types that represent a sequence
of data rather than a single scalar value; `glob` is also related, matching a whole set
of paths rather than naming a single one.",

            ValueType::TableInputStream(_) =>
                    "A table_input_stream is produced by any streaming command -- for example, the
output of `files` or `seq` -- or by reading the `read` member of a pipe object (see
`(table_input_stream ...):pipe`, under `help pipe`).

It can only be traversed once: each row is consumed as it's read, so a second pass over
the same stream sees nothing. If you need to read the same rows more than once, pipe the
stream through `materialize` to turn it into a reusable `table`. `table_output_stream` is
the writable counterpart of the same rows.",

            ValueType::TableOutputStream(_) =>
                    "A table_output_stream is obtained from the `output` member of a pipe object,
created by calling `:pipe` on a `table_input_stream` type, e.g.
`$p := $($(table_input_stream value=$integer):pipe)`; `$p:output` is then a
table_output_stream that rows can be written to, and `$p:read` is the matching
table_input_stream those same rows can be read back from.

Rows are written with the `write` method (see `help pipe:write`). The most similar type
is `table_input_stream`, its read-side counterpart.",

            ValueType::Table(_) =>
                    "A table is created by piping a table_input_stream through `materialize`, e.g.
`files | materialize`.

Unlike a table_input_stream, a table is a fixed snapshot: it can be read more than once,
indexed by row number (`$t[0]`), and asked for its length (`$t:len`) -- but it has no
methods for adding, removing, or replacing rows. The most similar type is
`table_input_stream`, the one-shot, streaming form it's materialized from.",

            ValueType::Struct => "To create a simple immutable struct, use the `struct:of` command,
e.g. `struct:of x=1 y=2`; its fields can be read (`$s:x`) but not reassigned.

To create a mutable struct that supports inheritance and methods, use the `class`
command; instances created from a class (`$MyClass:new ...`) do support field
reassignment (`$instance:x = 5`) from outside the class as well as from within its own
methods.

The most similar type is `dict`, which is also a mapping from keys to values, but with
keys chosen at runtime rather than fixed named fields, and no support for methods or
inheritance.",

            ValueType::List(_) => "Create a list with the `list:of` command, e.g. `list:of 1 2 3`,
or by collecting a column out of piped table input with `list:collect`.

Lists are mutable: elements can be appended, removed, or replaced in place. The most
similar type is `table`, which is also an ordered sequence of items but where each item
is a row of several named, differently-typed columns rather than a single value.",

            ValueType::Dict(_, _) => "Create a dict with the `dict:of` command, e.g.
`dict:of a=1 b=2`, or by collecting key/value columns out of piped table input with
`dict:collect`.

Dicts are mutable: entries can be inserted, removed, or have their value replaced in
place. The most similar type is `struct`, which is also a mapping from keys to values,
but with a fixed set of keys chosen when the struct is created rather than a dynamic,
mutable set of keys.",

            ValueType::Scope => "A scope is normally not constructed directly -- crush creates one
implicitly for the root namespace (`$global`) and for every closure or block invocation.
The scope currently executing can be obtained by calling `__current_scope__` on any
existing scope value, e.g. `$global:__current_scope__`.

Scopes are mutable: declaring a new variable (`:=`) or assignment (`=`) modifies the
scope it's declared or resolved in.",

            ValueType::Empty => "The instance of the empty type is returned by commands that don't
return any value, e.g. `echo`. There is no way to construct it directly -- it only ever
shows up as the natural result of a command that produces no output.",

            ValueType::Any => "This is a wildcard type, matching a value of any other type. It
shows up as the declared type of a column or argument that intentionally imposes no type
restriction -- it is not a type user code constructs values of directly.",

            ValueType::BinaryInputStream =>
                    "A binary_stream is obtained by reading binary data without fully loading it into
memory first, e.g. `bin:from ./Cargo.toml`, or the `body` of an HTTP response from
`io:http`.

Like `table_input_stream`, it can only be read once. Pipe it through `materialize` to
get a reusable `binary` value instead.",

            ValueType::Binary =>
                    "A binary value can be created by converting another value, e.g.
`convert $binary \"hi\"`, or by reading a `binary_stream` to completion with
`materialize`, e.g. `bin:from ./Cargo.toml | materialize`.

Binary data is immutable once created, and unlike `string` or `file` it has no legality
constraint at all -- it's simply an arbitrary sequence of bytes, the type to reach for
when data isn't necessarily text or a path. See `help string` for how the three
sequence-of-data types (`string`, `file`, `binary`) differ. The most similar type is
`binary_stream`, its one-shot streaming form.",

            ValueType::Type => "A type value names one of crush's own types, e.g. `$string` or
`$integer` -- `typeof` returns a value of this kind. Types are mostly used to declare
what kind of value a signature parameter or table column accepts, e.g. via `convert` or
a class field declaration, rather than being manipulated directly by everyday scripts.",

            ValueType::OneOf(_) => "A one_of value is constructed with the `one_of:of` command,
e.g. `one_of:of $file $string $regex`, and names a set of acceptable types rather than a
single one. It's used the same way an ordinary type is -- most commonly to declare that a
signature parameter accepts any one of several types -- rather than being a type user
code creates values of.",
        }.to_string()
            ];

        let mut keys: Vec<_> = self.fields().into_iter().collect();
        keys.sort_by(|x, y| x.0.cmp(&y.0));

        long_help_methods(&keys, &mut lines);
        Some(lines.join("\n"))
    }
}

fn long_help_methods(fields: &Vec<(&String, &Command)>, lines: &mut Vec<String>) {
    for (k, v) in fields {
        lines.push(format!(" * `{}` {}", k, v.short_help()));
    }
}

impl Display for ValueType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueType::String => f.write_str("string"),
            ValueType::Integer => f.write_str("integer"),
            ValueType::Time => f.write_str("time"),
            ValueType::Duration => f.write_str("duration"),
            ValueType::Glob => f.write_str("glob"),
            ValueType::Regex => f.write_str("re"),
            ValueType::Command => f.write_str("command"),
            ValueType::File => f.write_str("file"),
            ValueType::TableInputStream(columns) => {
                f.write_str("table_input_stream")?;
                for i in columns.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::TableOutputStream(columns) => {
                f.write_str("table_output_stream")?;
                for i in columns.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::Table(columns) => {
                f.write_str("table")?;
                for i in columns.iter() {
                    f.write_str(" ")?;
                    i.fmt(f)?;
                }
                Ok(())
            }
            ValueType::Struct => f.write_str("struct"),
            ValueType::List(value_type) => {
                f.write_str("list ")?;
                value_type.subfmt(f)
            }
            ValueType::Dict(key_type, value_type) => {
                f.write_str("dict ")?;
                key_type.subfmt(f)?;
                f.write_str(" ")?;
                value_type.subfmt(f)
            }
            ValueType::Scope => f.write_str("scope"),
            ValueType::Bool => f.write_str("bool"),
            ValueType::Float => f.write_str("float"),
            ValueType::Empty => f.write_str("empty"),
            ValueType::Any => f.write_str("any"),
            ValueType::BinaryInputStream => f.write_str("binary_stream"),
            ValueType::Binary => f.write_str("binary"),
            ValueType::Type => f.write_str("type"),
            ValueType::OneOf(types) => {
                f.write_str("one_of")?;
                for i in types.iter() {
                    f.write_str(" ")?;
                    i.subfmt(f)?;
                }
                Ok(())
            }
        }
    }
}
