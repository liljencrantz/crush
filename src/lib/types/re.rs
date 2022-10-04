use std::collections::HashSet;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::command::TypeMap;
use crate::lang::errors::{argument_error_legacy, CrushResult, error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use regex::Regex;
use signature::signature;
use crate::data::table::ColumnType;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::this::This;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "re", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "re"];
        res.declare(
            full("match"),
            r#match,
            false,
            "re =~ io:string",
            "True if the io matches the pattern",
            None,
            Known(ValueType::Bool),
            [],
        );
        res.declare(
            full("not_match"),
            not_match,
            false,
            "re !~ io:string",
            "True if the io does not match the pattern",
            None,
            Known(ValueType::Bool),
            [],
        );
        ReplaceSignature::declare_method(&mut res, &path);
        ReplaceAllSignature::declare_method(&mut res, &path);
        Filter::declare_method(&mut res, &path);
        res.declare(
            full("new"),
            new,
            false,
            "re:new pattern:string",
            "Create a new regular expression instance",
            None,
            Known(ValueType::Regex),
            [],
        );
        res
    };
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    let def = context.arguments.string(0)?;
    let res = match Regex::new(def.as_ref()) {
        Ok(r) => Value::Regex(def, r),
        Err(e) => return argument_error_legacy(e.to_string().as_str()),
    };
    context.output.send(res)
}

fn r#match(mut context: CommandContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(re.is_match(&needle)))
}

fn not_match(mut context: CommandContext) -> CrushResult<()> {
    let re = context.this.re()?.1;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(!re.is_match(&needle)))
}

#[signature(
    replace,
    can_block = false,
    short = "Replace the first match of the regex in text with the replacement",
    long = "re\"[0-9]\":replace \"123-456\" \"X\""
)]
struct ReplaceSignature {
    #[description("the text to perform replacement on.")]
    text: String,
    #[description("the replacement")]
    replacement: String,
}

#[signature(
    replace_all,
    can_block = false,
    short = "Replace all matches of the regex in text with the replacement",
    long = "re\"[0-9]\":replace \"123-456\" \"X\""
)]
struct ReplaceAllSignature {
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
filter,
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
                yas.contains(&column.name)
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
            let output = context.output.initialize(input.types().to_vec())?;
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
