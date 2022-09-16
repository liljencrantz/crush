use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error_legacy, CrushResult};
use crate::lang::state::contexts::{ArgumentVector, CommandContext, This};
use crate::lang::value::Value;
use crate::lang::{data::list::List, value::ValueType};
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use signature::signature;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "string", name]
}

mod format;

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> =
        {
            let mut res: OrderedMap<String, Command> = OrderedMap::new();
            let path = vec!["global", "types", "string"];
            Lower::declare_method(&mut res, &path);
            Upper::declare_method(&mut res, &path);
            Repeat::declare_method(&mut res, &path);
            Split::declare_method(&mut res, &path);
            Trim::declare_method(&mut res, &path);
            res.declare(
                full("format"),
                format::format,
                false,
                "string:format pattern:string [parameters:any]...",
                "Format arguments into a string",
                None,
                Known(ValueType::String),
                [],
            );
            Join::declare_method(&mut res, &path);
            LPad::declare_method(&mut res, &path);
            RPad::declare_method(&mut res, &path);
            StartsWith::declare_method(&mut res, &path);
            EndsWith::declare_method(&mut res, &path);
            IsAlphanumeric::declare_method(&mut res, &path);
            IsAlphabetic::declare_method(&mut res, &path);
            IsAscii::declare_method(&mut res, &path);
            IsLowercase::declare_method(&mut res, &path);
            IsUppercase::declare_method(&mut res, &path);
            IsWhitespace::declare_method(&mut res, &path);
            IsControl::declare_method(&mut res, &path);
            Len::declare_method(&mut res, &path);
            IsDigit::declare_method(&mut res, &path);
            Substr::declare_method(&mut res, &path);
            GetItem::declare_method(&mut res, &path);
            res
        };
}

#[signature(
    len, can_block=false, output=Known(ValueType::Integer),
    short="Returns the length (in number of characters) of the string")]
struct Len {}

fn len(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(context.this.string()?.len() as i128))
}

#[signature(
    upper, can_block=false, output=Known(ValueType::String),
    short="Returns an identical string but in upper case")]
struct Upper {}

fn upper(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::String(context.this.string()?.to_uppercase()))
}

#[signature(
    lower, can_block=false, output=Known(ValueType::String),
    short="Returns an identical string but in lower case")]
struct Lower {}

fn lower(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::String(context.this.string()?.to_lowercase()))
}

#[signature(
split,
can_block = false,
output=Known(ValueType::List(Box::from(ValueType::String))),
short = "Splits a string using the specified separator",
)]
struct Split {
    #[description("the separator to split on.")]
    separator: String,
}

fn split(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Split = Split::parse(context.arguments, &context.global_state.printer())?;
    let this = context.this.string()?;

    context.output.send(List::new(
        ValueType::String,
        this.split(&cfg.separator).map(|s| Value::string(s)).collect::<Vec<_>>(),
    ).into())
}

#[signature(
trim, can_block=false, output=Known(ValueType::String),
short="Returns a string with all whitespace trimmed from both ends")]
struct Trim {}

fn trim(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::string(context.this.string()?.trim()))
}

#[signature(
join,
can_block = false,
output=Known(ValueType::String),
short = "Join all arguments by the specified string",
example = "\", \":join 1 2 3 4 # 1, 2, 3, 4",
)]
struct Join {
    #[unnamed()]
    #[description("the elements to join.")]
    elements: Vec<Value>,
}

fn join(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Join = Join::parse(context.arguments, &context.global_state.printer())?;
    let s = context.this.string()?;
    let mut res = String::new();
    let mut first = true;

    for el in cfg.elements {
        if first {
            first = false;
        } else {
            res.push_str(&s);
        }
        res.push_str(&el.to_string());
    }

    context.output.send(Value::String(res))
}

#[signature(
    lpad,
    can_block = false,
    output=Known(ValueType::String),
    short = "Returns a string truncated or left-padded to be the exact specified length"
)]
struct LPad {
    #[description("the length to pad to.")]
    length: i128,
    #[description("the character to pad with.")]
    #[default(" ")]
    padding: String,
}

fn lpad(mut context: CommandContext) -> CrushResult<()> {
    let cfg: LPad = LPad::parse(context.arguments, &context.global_state.printer())?;
    let s = context.this.string()?;
    let len = cfg.length as usize;
    if cfg.padding.len() != 1 {
        argument_error_legacy("Padding string must be exactly one character long")
    } else if len <= s.len() {
        context.output.send(Value::string(&s[0..len]))
    } else {
        let mut res = cfg.padding.repeat(len - s.len());
        res += s.as_ref();
        context.output.send(Value::string(res.as_str()))
    }
}

#[signature(
    rpad,
    can_block = false,
    output=Known(ValueType::String),
    short = "Returns a string truncated or right-padded to be the exact specified length"
)]
struct RPad {
    #[description("the length to pad to.")]
    length: i128,
    #[description("the character to pad with.")]
    #[default(" ")]
    padding: String,
}

fn rpad(mut context: CommandContext) -> CrushResult<()> {
    let cfg: RPad = RPad::parse(context.arguments, &context.global_state.printer())?;
    let s = context.this.string()?;
    let len = cfg.length as usize;
    if cfg.padding.len() != 1 {
        argument_error_legacy("Padding string must be exactly one character long")
    } else if len <= s.len() {
        context.output.send(Value::string(&s[0..len]))
    } else {
        let mut res = s.to_string();
        res += cfg.padding.repeat(len - s.len()).as_str();
        context.output.send(Value::string(res.as_str()))
    }
}

#[signature(
repeat,
can_block = false,
output=Known(ValueType::String),
short = "Returns this string repeated times times"
)]
struct Repeat {
    #[description("the number of times to repeat the string.")]
    times: usize,
}

fn repeat(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Repeat = Repeat::parse(context.arguments, &context.global_state.printer())?;
    let s = context.this.string()?;
    context.output.send(Value::string(s.repeat(cfg.times).as_str()))
}

#[signature(
ends_with, can_block = false,
output=Known(ValueType::Bool),
short = "True if this string ends with the specified suffix",
)]
struct EndsWith {
    #[description("suffix to check for")]
    suffix: String,
}

fn ends_with(mut context: CommandContext) -> CrushResult<()> {
    let cfg: EndsWith = EndsWith::parse(context.arguments, &context.global_state.printer())?;
    let s = context.this.string()?;
    context.output.send(Value::Bool(s.ends_with(&cfg.suffix)))
}

#[signature(
starts_with, can_block = false,
output=Known(ValueType::Bool),
short = "True if this string starts with the specified prefix",
)]
struct StartsWith {
    #[description("prefix to check for")]
    prefix: String,
}

fn starts_with(mut context: CommandContext) -> CrushResult<()> {
    let cfg: StartsWith = StartsWith::parse(context.arguments, &context.global_state.printer())?;
    let s = context.this.string()?;
    context.output.send(Value::Bool(s.starts_with(&cfg.prefix)))
}

macro_rules! per_char_method {

    ($name:ident, $test:expr) => {
        fn $name(mut context: CommandContext) -> CrushResult<()> {
            context.arguments.check_len(0)?;
            let s = context.this.string()?;
            context.output.send(Value::Bool(s.chars().all($test)))
        }
    };
}

#[signature(
    is_alphanumeric, can_block = false, output=Known(ValueType::Bool),
    short = "True if every character of this string is alphabetic or numeric (assuming radix 10)",
)]
struct IsAlphanumeric {}

per_char_method!(is_alphanumeric, |ch| ch.is_alphanumeric());

#[signature(
is_alphabetic, can_block = false, output=Known(ValueType::Bool),
short = "True if every character of this string is alphabetic
",
)]
struct IsAlphabetic {}
per_char_method!(is_alphabetic, |ch| ch.is_alphabetic());

#[signature(
is_ascii, can_block = false, output=Known(ValueType::Bool),
short = "True if every character of this string is part of the ascii character set",
)]
struct IsAscii {}
per_char_method!(is_ascii, |ch| ch.is_ascii());

#[signature(
is_lowercase, can_block = false, output=Known(ValueType::Bool),
short = "True if every character of this string is lower case",
)]
struct IsLowercase {}
per_char_method!(is_lowercase, |ch| ch.is_lowercase());

#[signature(
is_uppercase, can_block = false, output=Known(ValueType::Bool),
short = "True if every character of this string is upper case",
)]
struct IsUppercase {}
per_char_method!(is_uppercase, |ch| ch.is_uppercase());

#[signature(
is_whitespace, can_block = false, output=Known(ValueType::Bool),
short = "True if every character of this string is a whitespace character",
)]
struct IsWhitespace {}
per_char_method!(is_whitespace, |ch| ch.is_whitespace());

#[signature(
is_control, can_block = false, output=Known(ValueType::Bool),
short = "True if every character of this string is a control character",
)]
struct IsControl {}
per_char_method!(is_control, |ch| ch.is_control());

#[signature(
    is_digit,
    can_block = false,
    output=Known(ValueType::Bool),
    short = "True if every character of this string is a digit in the specified radix",
    long = "\"123\":is_digit # true"
)]
struct IsDigit {
    #[description("radix to use")]
    #[default(10usize)]
    radix: usize,
}

fn is_digit(mut context: CommandContext) -> CrushResult<()> {
    let cfg: IsDigit = IsDigit::parse(context.arguments, &context.global_state.printer())?;
    let s = context.this.string()?;
    context.output.send(Value::Bool(
        s.chars().all(|ch| ch.is_digit(cfg.radix as u32)),
    ))
}

#[signature(
substr,
can_block = false,
output=Known(ValueType::String),
short = "Extract a substring from this string.",
)]
struct Substr {
    #[description("Starting index (inclusive).")]
    #[default(0usize)]
    from: usize,
    #[description("ending index (exclusive). If unspecified, to end of string.")]
    to: Option<usize>,
}

fn substr(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Substr = Substr::parse(context.remove_arguments(), &context.global_state.printer())?;
    let s = context.this.string()?;
    let to = cfg.to.unwrap_or(s.len());

    if to < cfg.from {
        return argument_error_legacy("From larger than to");
    }
    if to > s.len() {
        return argument_error_legacy("Substring beyond end of string");
    }
    context
        .output
        .send(Value::string(&s[cfg.from..to]))
}

#[signature(
__getitem__,
can_block = false,
output=Known(ValueType::String),
short = "Extract a one character substring from this string.",
)]
struct GetItem {
    #[description("index.")]
    idx: usize,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.global_state.printer())?;
    let s = context.this.string()?;
    if cfg.idx >= s.len() {
        return argument_error_legacy("Index beyond end of string");
    }
    context
        .output
        .send(Value::string(&s[cfg.idx..(cfg.idx+1)]))
}
