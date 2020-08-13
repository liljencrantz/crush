use crate::lang::argument::ArgumentHandler;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ArgumentVector, This};
use crate::lang::value::Value;
use crate::lang::{execution_context::ExecutionContext, list::List, value::ValueType};
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
            res.declare(
                full("upper"),
                upper,
                false,
                "string:upper",
                "Returns an identical string but in upper case",
                None,
                Known(ValueType::String),
            );
            res.declare(
                full("lower"),
                lower,
                false,
                "string:lower",
                "Returns an identical string but in lower case",
                None,
                Known(ValueType::String),
            );
            res.declare(
                full("repeat"),
                repeat,
                false,
                "string:repeat times:integer",
                "Returns this string repeated times times",
                None,
                Known(ValueType::String),
            );
            res.declare(
                full("split"),
                split,
                false,
                "string:split separator:string",
                "Splits a string using the specifiec separator",
                None,
                Known(ValueType::List(Box::from(ValueType::String))),
            );
            res.declare(
                full("trim"),
                trim,
                false,
                "string:trim",
                "Returns a string with all whitespace trimmed from both ends",
                None,
                Known(ValueType::String),
            );
            res.declare(
                full("format"),
                format::format,
                false,
                "string:format pattern:string [parameters:any]...",
                "Format arguments into a string",
                None,
                Known(ValueType::String),
            );
            // TODO: why unused?
            let _ = LPad::declare_method(&mut res, &path);
            let _ = RPad::declare_method(&mut res, &path);
            res.declare(
                full("ends_with"),
                ends_with,
                false,
                "string:ends_with suffix:string",
                "True if this string ends with suffix",
                None,
                Known(ValueType::Bool),
            );
            res.declare(
                full("starts_with"),
                starts_with,
                false,
                "string:starts_with prefix:string",
                "True if this string starts with prefix",
                None,
                Known(ValueType::Bool),
            );
            res.declare(full("is_alphanumeric"),
            is_alphanumeric, false,
            "string:is_alphanumeric",
            "True if every character of this string is alphabetic or numeric (assuming radix 10)",
            None,
            Known(ValueType::Bool));
            res.declare(
                full("is_alphabetic"),
                is_alphabetic,
                false,
                "string:is_alphabetic",
                "True if every character of this string is alphabetic",
                None,
                Known(ValueType::Bool),
            );
            res.declare(
                full("is_ascii"),
                is_ascii,
                false,
                "string:is_ascii",
                "True if every character of this string is part of the ascii character set",
                None,
                Known(ValueType::Bool),
            );
            res.declare(
                full("is_lowercase"),
                is_lowercase,
                false,
                "string:is_lowercase",
                "True if every character of this string is lower case",
                None,
                Known(ValueType::Bool),
            );
            res.declare(
                full("is_uppercase"),
                is_uppercase,
                false,
                "string:is_uppercase",
                "True if every character of this string is upper case",
                None,
                Known(ValueType::Bool),
            );
            res.declare(
                full("is_whitespace"),
                is_whitespace,
                false,
                "string:is_whitespace",
                "True if every character of this string is a whitespace character",
                None,
                Known(ValueType::Bool),
            );
            res.declare(
                full("is_control"),
                is_control,
                false,
                "string:is_control",
                "True if every character of this string is a control character",
                None,
                Known(ValueType::Bool),
            );
            // TODO: why unused?
            let _ = IsDigit::declare_method(&mut res, &path);
            res
        };
}

fn upper(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::String(context.this.string()?.to_uppercase()))
}

fn lower(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::String(context.this.string()?.to_lowercase()))
}

fn split(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.string()?;
    let separator = context.arguments.string(0)?;
    context.output.send(Value::List(List::new(
        ValueType::String,
        this.split(&separator).map(|s| Value::string(s)).collect(),
    )))
}

fn trim(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::string(context.this.string()?.trim()))
}

#[signature(
    lpad,
    can_block = false,
    short = "Returns a string truncated or left-padded to be the exact specified length"
)]
struct LPad {
    #[description("the length to pad to.")]
    length: i128,
    #[description("the character to pad with.")]
    #[default(" ")]
    padding: String,
}

fn lpad(context: ExecutionContext) -> CrushResult<()> {
    let cfg: LPad = LPad::parse(context.arguments, &context.printer)?;
    let s = context.this.string()?;
    let len = cfg.length as usize;
    if cfg.padding.len() != 1 {
        argument_error("Padding string must be exactly one character long")
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
    short = "Returns a string truncated or right-padded to be the exact specified length"
)]
struct RPad {
    #[description("the length to pad to.")]
    length: i128,
    #[description("the character to pad with.")]
    #[default(" ")]
    padding: String,
}

fn rpad(context: ExecutionContext) -> CrushResult<()> {
    let cfg: RPad = RPad::parse(context.arguments, &context.printer)?;
    let s = context.this.string()?;
    let len = cfg.length as usize;
    if cfg.padding.len() != 1 {
        argument_error("Padding string must be exactly one character long")
    } else if len <= s.len() {
        context.output.send(Value::string(&s[0..len]))
    } else {
        let mut res = s.to_string();
        res += cfg.padding.repeat(len - s.len()).as_str();
        context.output.send(Value::string(res.as_str()))
    }
}

fn repeat(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let s = context.this.string()?;
    let times = context.arguments.integer(0)? as usize;
    context.output.send(Value::string(s.repeat(times).as_str()))
}

fn ends_with(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let s = context.this.string()?;
    let suff = context.arguments.string(0)?;
    context.output.send(Value::Bool(s.ends_with(&suff)))
}

fn starts_with(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let s = context.this.string()?;
    let pre = context.arguments.string(0)?;
    context.output.send(Value::Bool(s.starts_with(&pre)))
}

macro_rules! per_char_method {
    ($name:ident, $test:expr) => {
        fn $name(context: ExecutionContext) -> CrushResult<()> {
            context.arguments.check_len(0)?;
            let s = context.this.string()?;
            context.output.send(Value::Bool(s.chars().all($test)))
        }
    };
}

per_char_method!(is_alphanumeric, |ch| ch.is_alphanumeric());
per_char_method!(is_alphabetic, |ch| ch.is_alphabetic());
per_char_method!(is_ascii, |ch| ch.is_ascii());
per_char_method!(is_lowercase, |ch| ch.is_lowercase());
per_char_method!(is_uppercase, |ch| ch.is_uppercase());
per_char_method!(is_whitespace, |ch| ch.is_whitespace());
per_char_method!(is_control, |ch| ch.is_control());

#[signature(
    is_digit,
    can_block = false,
    short = "True if every character of this string is a digit in the specified radix",
    long = "\"123\":is_digit # true"
)]
struct IsDigit {
    #[description("radix to use")]
    #[default(10usize)]
    radix: usize,
}

fn is_digit(context: ExecutionContext) -> CrushResult<()> {
    let cfg: IsDigit = IsDigit::parse(context.arguments, &context.printer)?;
    let s = context.this.string()?;
    context.output.send(Value::Bool(
        s.chars().all(|ch| ch.is_digit(cfg.radix as u32)),
    ))
}
