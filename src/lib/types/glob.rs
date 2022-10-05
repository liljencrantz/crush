use std::collections::HashSet;
use std::path::PathBuf;
use signature::signature;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Known;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::command::TypeMap;
use crate::lang::errors::{CrushResult, error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::data::list::List;
use crate::lang::value::ValueType;
use crate::lang::value::Value;
use crate::util::file::cwd;
use crate::util::glob::Glob;
use lazy_static::lazy_static;
use ordered_map::OrderedMap;
use crate::argument_error_legacy;
use crate::data::table::ColumnType;
use crate::lang::state::argument_vector::ArgumentVector;
use crate::lang::state::this::This;

fn full(name: &'static str) -> Vec<&'static str> {
    vec!["global", "types", "glob", name]
}

lazy_static! {
    pub static ref METHODS: OrderedMap<String, Command> = {
        let mut res: OrderedMap<String, Command> = OrderedMap::new();
        let path = vec!["global", "types", "glob"];
        res.declare(
            full("new"),
            new,
            false,
            "glob:new pattern:string",
            "Return a new glob",
            None,
            Known(ValueType::Glob),
            [],
        );
        res.declare(
            full("match"),
            r#match,
            false,
            "glob:match io:string",
            "True if the io matches the pattern",
            None,
            Known(ValueType::Bool),
            [],
        );
        res.declare(
            full("not_match"),
            not_match,
            false,
            "glob:not_match io:string",
            "True if the io does not match the pattern",
            None,
            Known(ValueType::Bool),
            [],
        );
        Files::declare_method(&mut res, &path);
        Filter::declare_method(&mut res, &path);
        res
    };
}

#[signature(
filter,
can_block = true,
output = Passthrough,
short = "Filter stream based on this glob.",
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
            .filter(|(_idx, column)| {
                match column.cell_type {
                    ValueType::File | ValueType::String => true,
                    _ => false,
                }
            })
            .map(|(idx, _c)| {idx})
            .collect()
    } else {
        let yas: HashSet<String> = cfg.drain(..).collect();
        input
            .iter()
            .enumerate()
            .filter(|(_idx, column)| {
                yas.contains(&column.name)
            })
            .map(|(idx, _c)| {idx})
            .collect()
    }
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

fn new(mut context: CommandContext) -> CrushResult<()> {
    let def = context.arguments.string(0)?;
    context.output.send(Value::Glob(Glob::new(&def)))
}

fn r#match(mut context: CommandContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(g.matches(&needle)))
}

fn not_match(mut context: CommandContext) -> CrushResult<()> {
    let g = context.this.glob()?;
    let needle = context.arguments.string(0)?;
    context.output.send(Value::Bool(!g.matches(&needle)))
}

#[signature(
files,
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
    context.output.send(List::new(
        ValueType::File,
        files.drain(..).map(|f| Value::from(f)).collect::<Vec<_>>(),
    ).into())
}
