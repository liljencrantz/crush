use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::find_string_columns;
use crate::lang::errors::{CrushResult, argument_error, command_error};
use crate::lang::signature::text::Text;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use ordered_map::OrderedMap;
use regex::Regex;
use signature::signature;
use std::sync::OnceLock;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        ReplaceSignature::declare_method(&mut res);
        ReplaceAllSignature::declare_method(&mut res);
        Filter::declare_method(&mut res);
        New::declare_method(&mut res);
        Match::declare_method(&mut res);
        NotMatch::declare_method(&mut res);

        res
    })
}

#[signature(
    types.re.new,
    can_block = false,
    output = Known(ValueType::Regex),
    short = "Compile a string into a new regular expression instance.",
)]
struct New {
    #[description("the new regular expression as a string.")]
    pattern: String,
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    let cfg: New = New::parse(context.remove_arguments(), &context.global_state.printer())?;
    let res = match Regex::new(&cfg.pattern) {
        Ok(r) => Value::Regex(cfg.pattern, r),
        Err(e) => return command_error(e.to_string()),
    };
    context.output.send(res)
}

#[signature(
    types.re.r#match,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if the io matches the pattern.",
)]
struct Match {
    #[description("the string to match against.")]
    needle: Text,
}

fn r#match(mut context: CommandContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let cfg: Match = Match::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::Bool(re.is_match(&cfg.needle.as_string())))
}

#[signature(
    types.re.not_match,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if the io matches the pattern.",
)]
struct NotMatch {
    #[description("the string to match against.")]
    needle: Text,
}

fn not_match(mut context: CommandContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let cfg: NotMatch =
        NotMatch::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::Bool(!re.is_match(&cfg.needle.as_string())))
}

#[signature(
    types.re.replace,
    can_block = false,
    short = "Replace the first match of the regex in text with the replacement",
    long = "re\"[0-9]\":replace \"123-456\" \"X\"",
)]
struct ReplaceSignature {
    #[description("the text to perform replacement on.")]
    text: String,
    #[description("the replacement")]
    replacement: String,
}

fn replace(mut context: CommandContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let args: ReplaceSignature =
        ReplaceSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::from(
        re.replace(&args.text, args.replacement.as_str()).as_ref(),
    ))
}

#[signature(
    types.re.replace_all,
    can_block = false,
    short = "Replace all matches of the regex in text with the replacement",
    long = "re\"[0-9]\":replace \"123-456\" \"X\"",
)]
struct ReplaceAllSignature {
    #[description("the text to perform replacement on.")]
    text: String,
    #[description("the replacement")]
    replacement: String,
}

fn replace_all(mut context: CommandContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let args: ReplaceAllSignature =
        ReplaceAllSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::from(
        re.replace_all(&args.text, args.replacement.as_str())
            .as_ref(),
    ))
}

#[signature(
    types.re.filter,
    can_block = true,
    output = Passthrough,
    short = "Filter input stream based on this regex.",
    long = "Search all textual (string and file) columns for matches of the regex, and output the rows that match.",
    example = "# Recursively search current directory for all files containing four `a` characters in a row",
    example = "files --recurse | ^(aaaa):filter",
)]
struct Filter {
    #[unnamed()]
    #[description(
        "Columns to filter on. Column must be textual. If no columns are specified, all textual columns are used."
    )]
    columns: Vec<String>,
}

pub fn filter(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Filter = Filter::parse(context.remove_arguments(), &context.global_state.printer())?;
    let re = context.this.re()?.1;
    let mut input = context.input.recv()?.stream()?;
    let columns = find_string_columns(input.types(), cfg.columns);
    let output = context.output.initialize(input.types())?;
    while let Ok(row) = input.read() {
        let mut found = false;
        for idx in &columns {
            match &row.cells()[*idx] {
                Value::String(s) => {
                    if re.is_match(&s) {
                        found = true;
                        break;
                    }
                }
                Value::File(s) => {
                    s.to_str().map(|s| {
                        if re.is_match(s) {
                            found = true;
                        }
                    });
                    if found {
                        break;
                    }
                }
                v => {
                    return argument_error(
                        format!(
                            "`re:filter`: Expected column `{}` to be `oneof $string $file`, but was `{}`",
                            input.types()[*idx].name(),
                            v.value_type(),
                        ),
                        &context.source,
                    );
                }
            }
        }
        if found {
            output.send(row)?;
        }
    }
    Ok(())
}
