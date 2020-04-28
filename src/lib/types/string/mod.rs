use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::{execution_context::ExecutionContext, value::ValueType, list::List};
use crate::lang::value::Value;
use crate::lang::execution_context::{This, ArgumentVector};
use std::collections::HashMap;
use lazy_static::lazy_static;
use crate::lang::command::CrushCommand;
use crate::lang::command::TypeMap;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "string", name]
}

mod format;

lazy_static! {
    pub static ref METHODS: HashMap<String, Box<dyn CrushCommand +  Sync + Send>> = {
        let mut res: HashMap<String, Box<dyn CrushCommand +  Send + Sync>> = HashMap::new();
        res.declare(
            full("upper"),
            upper, false,
            "string:upper",
            "Returns an identical string but in upper case",
            None);
        res.declare(full("lower"),
            lower, false,
            "string:lower",
            "Returns an identical string but in lower case",
            None);
        res.declare(full("repeat"),
            repeat, false,
            "string:repeat times:integer",
            "Returns this string repeated times times",
            None);
        res.declare(full("split"),
            split, false,
            "string:split separator:string",
            "Splits a string using the specifiec separator",
            None);
        res.declare(full("trim"),
            trim, false,
            "string:trim",
            "Returns a string with all whitespace trimmed from both ends",
            None);
        res.declare(full("format"),
            format::format, false,
            "string:format pattern:string [parameters:any]...",
            "Format arguments into a string",
            None);
        res.declare(full("lpad"),
            lpad, false,
            "string:lpad length [padding:string]",
            "Returns a string truncated or left-padded to be the exact specified length",
            None);
        res.declare(full("rpad"),
            rpad, false,
            "string:rpad length [padding:string]",
            "Returns a string truncated or right-padded to be the exact specified length",
            None);
        res.declare(full("ends_with"),
            ends_with, false,
            "string:ends_with suffix:string",
            "True if this string ends with suffix",
            None);
        res.declare(full("starts_with"),
            starts_with, false,
            "string:starts_with prefix:string",
            "True if this string starts with prefix",
            None);
        res.declare(full("is_alphanumeric"),
            is_alphanumeric, false,
            "string:is_alphanumeric",
            "True if every character of this string is alphabetic or numeric (assuming radix 10)",
            None);
        res.declare(full("is_alphabetic"),
            is_alphabetic, false,
            "string:is_alphabetic",
            "True if every character of this string is alphabetic",
            None);
        res.declare(full("is_ascii"),
            is_ascii, false,
            "string:is_ascii",
            "True if every character of this string is part of the ascii character set",
            None);
        res.declare(full("is_lowercase"),
            is_lowercase, false,
            "string:is_lowercase",
            "True if every character of this string is lower case",
            None);
        res.declare(full("is_uppercase"),
            is_uppercase, false,
            "string:is_uppercase",
            "True if every character of this string is upper case",
            None);
        res.declare(full("is_whitespace"),
            is_whitespace, false,
            "string:is_whitespace",
            "True if every character of this string is a whitespace character",
            None);
        res.declare(full("is_control"),
            is_control, false,
            "string:is_control",
            "True if every character of this string is a control character",
            None);
        res.declare(full("is_digit"),
            is_digit, false,
            "string:is_digit [radix:integer]",
            "True if every character of this string is a digit in the specified radix (default is 10)",
            None);
        res
    };
}

fn upper(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::String(
        context.this.string()?
            .to_uppercase()
            ))
}

fn lower(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::String(
        context.this.string()?
            .to_lowercase()
            ))
}

fn split(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(1)?;
    let this = context.this.string()?;
    let separator = context.arguments.string(0)?;
    context.output.send(Value::List(List::new(
        ValueType::String,
        this.split(&separator)
            .map(|s| Value::string(s))
            .collect())))
}

fn trim(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::string(context.this.string()?.trim()))
}

fn lpad(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len_range(1, 2)?;
    let s = context.this.string()?;
    let len = context.arguments.integer(0)? as usize;
    let pad_char = context.arguments.optional_string(1)?.unwrap_or_else(|| " ".to_string());
    if pad_char.len() != 1 {
        return argument_error("Padding string must be exactly one character long");
    }
    if len <= s.len() {
        context.output.send(Value::string(
            &s[0..len]))
    } else {
        let mut res = pad_char.repeat(len - s.len());
        res += s.as_ref();
        context.output.send(Value::string(res.as_str()))
    }
}

fn rpad(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len_range(1, 2)?;
    let s = context.this.string()?;
    let len = context.arguments.integer(0)? as usize;
    let pad_char = context.arguments.optional_string(1)?.unwrap_or_else(|| " ".to_string());
    if pad_char.len() != 1 {
        return argument_error("Padding string must be exactly one character long");
    }
    if len <= s.len() {
        context.output.send(Value::string(
            &s[0..len]))
    } else {
        let mut res = s.to_string();
        res += pad_char.repeat(len - s.len()).as_str();
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
    }
}

per_char_method!(is_alphanumeric, |ch| ch.is_alphanumeric());
per_char_method!(is_alphabetic, |ch| ch.is_alphabetic());
per_char_method!(is_ascii, |ch| ch.is_ascii());
per_char_method!(is_lowercase, |ch| ch.is_lowercase());
per_char_method!(is_uppercase, |ch| ch.is_uppercase());
per_char_method!(is_whitespace, |ch| ch.is_whitespace());
per_char_method!(is_control, |ch| ch.is_control());

fn is_digit(mut context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len_range(0, 1)?;
    let s = context.this.string()?;
    let radix = context.arguments.optional_integer(0)?.unwrap_or(10i128) as u32;
    context.output.send(Value::Bool(s.chars().all(|ch| ch.is_digit(radix))))
}
