use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::value::{Value, ValueType};
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use signature::signature;

/// The RFC 3986 `unreserved` set (ASCII letters, digits, and `-_.~`) is the only set of
/// bytes left untouched; everything else -- including bytes that are only reserved in
/// some URL components, like `/` and `&` -- is escaped. This is the strictest common
/// choice, correct for a single path segment, query key, or query value.
const PERCENT_ENCODE_SET: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

fn read_string(context: &mut CommandContext) -> CrushResult<String> {
    match context.input.recv()? {
        Value::String(s) => Ok(s.to_string()),
        v => command_error(format!(
            "Expected a string, encountered `{}`.",
            v.value_type().to_string()
        )),
    }
}

#[signature(
    io.percent.to,
    can_block = false,
    output = Known(ValueType::String),
    short = "Percent-encode a string for safe use in a URL.",
    long = "Escapes every byte except the RFC 3986 `unreserved` set -- ASCII letters,",
    long = "digits, and `-_.~` -- as `%XX`. This is the strictest common encoding, safe to",
    long = "use for a single path segment, query key, or query value. It does escape `/`",
    long = "and `&`, so don't apply it to an already-assembled path or query string, only",
    long = "to one component of one.",
    example = "# Returns \"a%20b%2Fc\"",
    example = "percent:to \"a b/c\"",
)]
struct To {
    #[unnamed()]
    #[description("the string to percent-encode. Reads a string from input if unspecified.")]
    input: Option<String>,
}

pub fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg = To::parse(context.remove_arguments(), &context.global_state.printer())?;
    let input = match cfg.input {
        Some(s) => s,
        None => read_string(&mut context)?,
    };
    let encoded = utf8_percent_encode(&input, PERCENT_ENCODE_SET).to_string();
    context.output.send(Value::from(encoded))
}

#[signature(
    io.percent.from,
    can_block = false,
    output = Known(ValueType::String),
    short = "Decode a percent-encoded string.",
    long = "Replaces every `%XX` escape with the byte it represents. Errors if the",
    long = "decoded bytes aren't valid UTF-8.",
    example = "# Returns \"a b/c\"",
    example = "percent:from \"a%20b%2Fc\"",
)]
struct From {
    #[unnamed()]
    #[description("the percent-encoded string to decode. Reads a string from input if unspecified.")]
    input: Option<String>,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg = From::parse(context.remove_arguments(), &context.global_state.printer())?;
    let input = match cfg.input {
        Some(s) => s,
        None => read_string(&mut context)?,
    };
    let decoded = percent_decode_str(&input).decode_utf8()?.into_owned();
    context.output.send(Value::from(decoded))
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "percent",
        "Percent-encoding (URL-style) conversions",
        Box::new(move |env| {
            To::declare(env)?;
            From::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
