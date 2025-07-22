use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::signature::text::Text;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::{data::list::List, value::ValueType};
use ordered_map::OrderedMap;
use signature::signature;
use std::sync::OnceLock;

mod format;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();

        Lower::declare_method(&mut res);
        Upper::declare_method(&mut res);
        Repeat::declare_method(&mut res);
        Split::declare_method(&mut res);
        Trim::declare_method(&mut res);
        format::Format::declare_method(&mut res);
        Join::declare_method(&mut res);
        LPad::declare_method(&mut res);
        RPad::declare_method(&mut res);
        StartsWith::declare_method(&mut res);
        EndsWith::declare_method(&mut res);
        IsAlphanumeric::declare_method(&mut res);
        IsAlphabetic::declare_method(&mut res);
        IsAscii::declare_method(&mut res);
        IsLowercase::declare_method(&mut res);
        IsUppercase::declare_method(&mut res);
        IsWhitespace::declare_method(&mut res);
        IsControl::declare_method(&mut res);
        Len::declare_method(&mut res);
        Bytes::declare_method(&mut res);
        IsDigit::declare_method(&mut res);
        Substr::declare_method(&mut res);
        GetItem::declare_method(&mut res);
        Match::declare_method(&mut res);
        NotMatch::declare_method(&mut res);

        res
    })
}

#[signature(
    types.string.len,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the length (in number of unicode characters) of the string.",
    long = "Note that a unicode character is not always what a human would consider a character, and there are often different ways to generate identical looking strings with different numbers of characters, as in the example below.",
    example = "# Returns 1",
    example = "\"é\":len",
    example = "# Returns 2",
    example = "\"é\":len",
)]
struct Len {}

fn len(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(context.this.string(&context.source)?.chars().count() as i128))
}

#[signature(
    types.string.bytes,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "Returns the length (in number of bytes) of the string, as encoded using UTF-8",
    long = "Note that there are often different ways to generate identical looking strings of different lengths, as in the example below.",
    example = "# Returns 2",
    example = "\"é\":bytes",
    example = "# Returns 3",
    example = "\"é\":bytes",
)]
struct Bytes {}

fn bytes(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(context.this.string(&context.source)?.len() as i128))
}

#[signature(
    types.string.upper, can_block = false, output = Known(ValueType::String),
    short = "Returns an identical string but in upper case")]
struct Upper {}

fn upper(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::from(context.this.string(&context.source)?.to_uppercase()))
}

#[signature(
    types.string.lower, can_block = false, output = Known(ValueType::String),
    short = "Returns an identical string but in lower case")]
struct Lower {}

fn lower(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::from(context.this.string(&context.source)?.to_lowercase()))
}

#[signature(
    types.string.split,
    can_block = false,
    output = Known(ValueType::List(Box::from(ValueType::String))),
    short = "Splits a string using the specified separator",
    example = "# Returns [\"Hello\", \"World\"]",
    example = "\"Hello World\":split \" \"",
)]
struct Split {
    #[description("the separator to split on.")]
    separator: String,
}

fn split(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Split = Split::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let this = context.this.string(&context.source)?;

    context.output.send(
        List::new(
            ValueType::String,
            this.split(&cfg.separator)
                .map(|s| Value::from(s))
                .collect::<Vec<_>>(),
        )
        .into(),
    )
}

#[signature(
    types.string.trim, 
    can_block = false, 
    output = Known(ValueType::String),
    short = "Returns a string with all whitespace trimmed from both ends")]
struct Trim {}

fn trim(mut context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::from(context.this.string(&context.source)?.trim()))
}

#[signature(
    types.string.join,
    can_block = false,
    output = Known(ValueType::String),
    short = "Join all arguments by the specified string",
    example = "# Returns \"1, 2, 3, 4\"",
    example = "\", \":join 1 2 3 4",
)]
struct Join {
    #[unnamed()]
    #[description("the elements to join.")]
    elements: Vec<Value>,
}

fn join(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Join = Join::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
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

    context.output.send(Value::from(res))
}

#[signature(
    types.string.lpad,
    can_block = false,
    output = Known(ValueType::String),
    short = "Returns a string truncated or left-padded to be the exact specified length",
    example = "# Returns \"     Hello\"",
    example = "\"Hello\":lpad 10",
    example = "# Returns \"He\"",
    example = "\"Hello\":lpad 2",
)]
struct LPad {
    #[description("the length to pad to.")]
    length: i128,
    #[description("the character to pad with.")]
    #[default(" ")]
    padding: String,
}

fn lpad(mut context: CommandContext) -> CrushResult<()> {
    let cfg: LPad = LPad::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
    let len = cfg.length as usize;
    if cfg.padding.len() != 1 {
        argument_error("Padding string must be exactly one character long.", &context.source)
    } else if len <= s.len() {
        context.output.send(Value::from(&s[0..len]))
    } else {
        let mut res = cfg.padding.repeat(len - s.len());
        res += s.as_ref();
        context.output.send(Value::from(res.as_str()))
    }
}

#[signature(
    types.string.rpad,
    can_block = false,
    output = Known(ValueType::String),
    short = "Returns a string truncated or right-padded to be the exact specified length",
    example = "# Returns \"Helloooooo\"",
    example = "\"Hello\":rpad length=10 padding=o",
    example = "# Returns \"He\"",
    example = "\"Hello\":rpad 2",
)]
struct RPad {
    #[description("the length to pad to.")]
    length: i128,
    #[description("the character to pad with.")]
    #[default(" ")]
    padding: String,
}

fn rpad(mut context: CommandContext) -> CrushResult<()> {
    let cfg: RPad = RPad::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
    let len = cfg.length as usize;
    if cfg.padding.len() != 1 {
        argument_error("Padding string must be exactly one character long.", &context.source)
    } else if len <= s.len() {
        context.output.send(Value::from(&s[0..len]))
    } else {
        let mut res = s.to_string();
        res += cfg.padding.repeat(len - s.len()).as_str();
        context.output.send(Value::from(res.as_str()))
    }
}

#[signature(
    types.string.repeat,
    can_block = false,
    output = Known(ValueType::String),
    short = "Returns this string repeated times times",
    example = "\"Around the world\\n\":repeat 8",
)]
struct Repeat {
    #[description("the number of times to repeat the string.")]
    times: usize,
}

fn repeat(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Repeat = Repeat::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
    context
        .output
        .send(Value::from(s.repeat(cfg.times).as_str()))
}

#[signature(
    types.string.ends_with, can_block = false,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if this string ends with the specified suffix",
)]
struct EndsWith {
    #[description("suffix to check for")]
    suffix: String,
}

fn ends_with(mut context: CommandContext) -> CrushResult<()> {
    let cfg: EndsWith = EndsWith::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
    context.output.send(Value::Bool(s.ends_with(&cfg.suffix)))
}

#[signature(
    types.string.starts_with, can_block = false,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if this string starts with the specified prefix",
)]
struct StartsWith {
    #[description("prefix to check for")]
    prefix: String,
}

fn starts_with(mut context: CommandContext) -> CrushResult<()> {
    let cfg: StartsWith = StartsWith::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
    context.output.send(Value::Bool(s.starts_with(&cfg.prefix)))
}

macro_rules! per_char_method {
    ($name:ident, $test:expr) => {
        fn $name(mut context: CommandContext) -> CrushResult<()> {
            context.arguments.check_len(0)?;
            let s = context.this.string(&context.source)?;
            context.output.send(Value::Bool(s.chars().all($test)))
        }
    };
}

#[signature(
    types.string.is_alphanumeric,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is alphabetic or numeric",
)]
struct IsAlphanumeric {}

per_char_method!(is_alphanumeric, |ch| ch.is_alphanumeric());

#[signature(
    types.string.is_alphabetic,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is alphabetic",
)]
struct IsAlphabetic {}
per_char_method!(is_alphabetic, |ch| ch.is_alphabetic());

#[signature(
    types.string.is_ascii,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is part of the ascii character set",
)]
struct IsAscii {}
per_char_method!(is_ascii, |ch| ch.is_ascii());

#[signature(
    types.string.is_lowercase,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is lower case",
)]
struct IsLowercase {}
per_char_method!(is_lowercase, |ch| ch.is_lowercase());

#[signature(
    types.string.is_uppercase,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is upper case",
)]
struct IsUppercase {}
per_char_method!(is_uppercase, |ch| ch.is_uppercase());

#[signature(
    types.string.is_whitespace,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is a whitespace character",
)]
struct IsWhitespace {}
per_char_method!(is_whitespace, |ch| ch.is_whitespace());

#[signature(
    types.string.is_control,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is a control character",
)]
struct IsControl {}
per_char_method!(is_control, |ch| ch.is_control());

#[signature(
    types.string.is_digit,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if every character of this string is a digit in the specified radix",
    long = "\"123\":is_digit # true"
)]
struct IsDigit {
    #[description("radix to use")]
    #[default(10usize)]
    radix: usize,
}

fn is_digit(mut context: CommandContext) -> CrushResult<()> {
    let s = context.this.string(&context.source)?;
    let cfg: IsDigit = IsDigit::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    context.output.send(Value::Bool(
        s.chars().all(|ch| ch.is_digit(cfg.radix as u32)),
    ))
}

#[signature(
    types.string.substr,
    can_block = false,
    output = Known(ValueType::String),
    short = "Extract a substring from this string.",
)]
struct Substr {
    #[description("Starting index (inclusive). If unspecified, from start of string.")]
    #[default(0usize)]
    from: usize,
    #[description("ending index (exclusive). If unspecified, to end of string.")]
    to: Option<usize>,
}

fn substr(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Substr = Substr::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
    let to = cfg.to.unwrap_or(s.len());

    if to < cfg.from {
        return argument_error("The `from` index must not be larger than the `to` index.", &context.source);
    }
    if to > s.len() {
        return argument_error("Substring goes beyond end of string", &context.source);
    }
    context.output.send(Value::from(&s[cfg.from..to]))
}

#[signature(
    types.string.__getitem__,
    can_block = false,
    output = Known(ValueType::String),
    short = "Extract a one character substring from this string.",
)]
struct GetItem {
    #[description("index")]
    idx: usize,
}

fn __getitem__(mut context: CommandContext) -> CrushResult<()> {
    let cfg: GetItem = GetItem::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    let s = context.this.string(&context.source)?;
    if cfg.idx >= s.len() {
        return argument_error("Index beyond end of string", &context.source);
    }
    context.output.send(Value::from(&s[cfg.idx..(cfg.idx + 1)]))
}

#[signature(
    types.string.r#match,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if the needle matches the pattern",
)]
struct Match {
    #[description("the text to match this string against.")]
    needle: Text,
}

fn r#match(mut context: CommandContext) -> CrushResult<()> {
    let s = context.this.string(&context.source)?;
    let cfg: Match = Match::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    context
        .output
        .send(Value::Bool(s.eq(&cfg.needle.as_string())))
}

#[signature(
    types.string.not_match,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "False if the needle matches the pattern",
)]
struct NotMatch {
    #[description("the text to match this glob against.")]
    needle: Text,
}

fn not_match(mut context: CommandContext) -> CrushResult<()> {
    let s = context.this.string(&context.source)?;
    let cfg: NotMatch =
        NotMatch::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    context
        .output
        .send(Value::Bool(!s.eq(&cfg.needle.as_string())))
}
