use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::list::List;
use crate::lang::errors::{CrushResult, error, argument_error};
use crate::lang::signature::text::Text;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::this::This;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::file::cwd;
use crate::util::glob::Glob;
use ordered_map::OrderedMap;
use signature::signature;
use std::path::PathBuf;
use std::sync::OnceLock;
use crate::lang::data::table::find_string_columns;

pub fn methods() -> &'static OrderedMap<String, Command> {
    static CELL: OnceLock<OrderedMap<String, Command>> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        New::declare_method(&mut res);
        Match::declare_method(&mut res);
        NotMatch::declare_method(&mut res);
        Files::declare_method(&mut res);
        Filter::declare_method(&mut res);

        res
    })
}

#[signature(
    types.glob.filter,
    can_block = true,
    output = Passthrough,
    short = "Filter input stream based on this glob.",
    long = "Search all textual (string and file) columns for matches of the glob, and output the rows that match.",
    example = "# Recursively search current directory for all files containing four `a` characters in a row",
    example = "files --recurse | *aaaa*:filter",
)]
struct Filter {
    #[unnamed()]
    #[description("Columns to filter on. Column must be textual. If no columns are specified, all textual columns are used.")]
    columns: Vec<String>,
}

pub fn filter(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Filter = Filter::parse(context.remove_arguments(), &context.global_state.printer())?;
    let glob = context.this.glob()?;
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let columns = find_string_columns(input.types(), cfg.columns);
            let output = context.output.initialize(input.types())?;
            while let Ok(row) = input.read() {
                let mut found = false;
                for idx in &columns {
                    match &row.cells()[*idx] {
                        Value::String(s) => {
                            if glob.matches(&s) {
                                found = true;
                                break;
                            }
                        }
                        Value::File(s) => {
                            s.to_str().map(|s| {
                                if glob.matches(s) {
                                    found = true;
                                }
                            });
                            if found {
                                break;
                            }
                        }
                        v => return argument_error(format!(
                            "`glob:filter`: Expected column `{}` to be `oneof $string $file`, but was `{}`",
                            input.types()[*idx].name(), v.value_type(),
                        ), &context.source),
                    }
                }
                if found {
                    output.send(row)?;
                }
            }
            Ok(())
        }
        None => error("`glob:filter`: Expected a stream"),
    }
}

#[signature(
    types.glob.new,
    can_block = false,
    output = Known(ValueType::Glob),
    short = "Create a glob from a string",
)]
struct New {
    #[description("the string representation of the glob.")]
    glob: String,
}

fn new(mut context: CommandContext) -> CrushResult<()> {
    let cfg: New = New::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Glob(Glob::new(&cfg.glob)))
}

#[signature(
    types.glob.r#match,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "True if the needle matches the pattern",
)]
struct Match {
    #[description("the text to match this glob against.")]
    needle: Text,
}

fn r#match(mut context: CommandContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let cfg: Match = Match::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::Bool(g.matches(&cfg.needle.as_string())))
}

#[signature(
    types.glob.not_match,
    can_block = false,
    output = Known(ValueType::Bool),
    short = "False if the needle matches the pattern",
)]
struct NotMatch {
    #[description("the text to match this glob against.")]
    needle: Text,
}

fn not_match(mut context: CommandContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let cfg: NotMatch =
        NotMatch::parse(context.remove_arguments(), &context.global_state.printer())?;
    context
        .output
        .send(Value::Bool(!g.matches(&cfg.needle.as_string())))
}

#[signature(
    types.glob.files,
    can_block = true,
    output = Known(ValueType::List(Box::from(ValueType::File))),
    short = "Perform file matching of this glob.",
)]
struct Files {
    #[description("the directory to match in. Use current working directory if unspecified.")]
    directory: Option<PathBuf>,
}

fn files(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Files = Files::parse(context.remove_arguments(), &context.global_state.printer())?;
    let g = context.this.glob()?;
    let mut files = Vec::new();
    g.glob_files(&cfg.directory.unwrap_or(cwd()?), &mut files)?;
    context.output.send(
        List::new(
            ValueType::File,
            files.drain(..).map(|f| Value::from(f)).collect::<Vec<_>>(),
        )
        .into(),
    )
}
