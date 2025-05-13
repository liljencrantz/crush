use std::collections::HashSet;
use std::sync::OnceLock;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{argument_error_legacy, CrushResult, error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use ordered_map::OrderedMap;
use regex::Regex;
use signature::signature;
use crate::data::table::ColumnType;
use crate::lang::signature::text::Text;
use crate::lang::state::this::This;

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
        Err(e) => return argument_error_legacy(e.to_string().as_str()),
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
    context.output.send(Value::Bool(re.is_match(&cfg.needle.as_string())))
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
    let cfg: NotMatch = NotMatch::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Bool(!re.is_match(&cfg.needle.as_string())))
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
    let args: ReplaceSignature = ReplaceSignature::parse(context.arguments, &context.global_state.printer())?;
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
        ReplaceAllSignature::parse(context.arguments, &context.global_state.printer())?;
    context.output.send(Value::from(
        re.replace_all(&args.text, args.replacement.as_str())
            .as_ref(),
    ))
}

#[signature(
    types.re.filter,
    can_block = true,
    output = Passthrough,
    short = "Filter stream based on this regex.",
)]
struct Filter {
    #[unnamed()]
    #[description("Columns to filter on")]
    columns: Vec<String>,
}

fn find_string_columns(input: &[ColumnType], mut cfg: Vec<String>) -> Vec<usize> {
    if cfg.is_empty() {
        input
            .iter()
            .enumerate()
            .filter(|(_, column)| {
                match column.cell_type {
                    ValueType::File | ValueType::String => true,
                    _ => false,
                }
            })
            .map(|(idx, _)| {idx})
            .collect()
    } else {
        let yas: HashSet<String> = cfg.drain(..).collect();
        input
            .iter()
            .enumerate()
            .filter(|(_, column)| {
                yas.contains(column.name())
            })
            .map(|(idx, _c)| {idx})
            .collect()
    }
}

pub fn filter(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Filter = Filter::parse(context.remove_arguments(), &context.global_state.printer())?;
    let re = context.this.re()?.1;
    match context.input.recv()?.stream()? {
        Some(mut input) => {
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
                        _ => return argument_error_legacy("Expected a string or file value"),
                    }
                }
                if found {
                    output.send(row)?;
                }
            }
            Ok(())
        }
        None => error("Expected a stream"),
    }
}
