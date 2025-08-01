use crate::lang::state::contexts::CommandContext;
use crate::{
    lang::errors::CrushError,
    lang::{data::table::Row, value::Value},
};
use std::{io::BufReader, io::prelude::*};

use crate::lang::data::table::ColumnType;
use crate::lang::errors::{CrushResult, error};

use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::binary_input::ToReader;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::value::ValueType;
use signature::signature;

#[signature(
    io.csv.from,
    example = "csv:from separator=\",\" head=1 name=$string age=$integer nick=$string",
    short = "Parse specified files as CSV files"
)]
struct From {
    #[unnamed()]
    #[description(
        "source. If unspecified, will read from input, which must be a binary or binary_stream."
    )]
    files: Vec<BinaryInput>,
    #[named()]
    #[description("name and type of all columns.")]
    columns: OrderedStringMap<ValueType>,
    #[description("column separator.")]
    #[default(',')]
    separator: char,
    #[default(0usize)]
    #[description("skip this many lines of input from the beginning.")]
    head: usize,
    #[description("trim this character from start and end of every value.")]
    trim: Option<char>,
}

fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.remove_arguments(), &context.global_state.printer())?;
    let columns = cfg
        .columns
        .iter()
        .map(|(k, v)| ColumnType::new_from_string(k.clone(), v.clone()))
        .collect::<Vec<_>>();
    let output = context.output.initialize(&columns)?;

    let mut reader = BufReader::new(cfg.files.to_reader(context.input)?);

    let separator = cfg.separator;
    let trim = cfg.trim;
    let skip = cfg.head;

    let mut line = String::new();
    let mut skipped = 0usize;
    loop {
        line.clear();
        reader.read_line(&mut line)?;
        if line.is_empty() {
            break;
        }
        if skipped < skip {
            skipped += 1;
            continue;
        }
        let line_without_newline = if line.ends_with('\n') {
            &line[0..line.len() - 1]
        } else {
            &line
        };

        let mut split: Vec<&str> = line_without_newline
            .split(separator)
            .map(|s| trim.map(|c| s.trim_matches(c)).unwrap_or(s))
            .collect();

        if split.len() != columns.len() {
            return error("Wrong number of columns in CSV file.");
        }

        if let Some(trim) = trim {
            split = split.iter().map(|s| s.trim_matches(trim)).collect();
        }

        match split
            .iter()
            .zip(columns.iter())
            .map(|(s, t)| t.cell_type.parse(*s))
            .collect::<Result<Vec<Value>, CrushError>>()
        {
            Ok(cells) => {
                let _ = output.send(Row::new(cells));
            }
            Err(err) => {
                return Err(err);
            }
        }
    }
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "csv",
        "CSV I/O",
        Box::new(move |env| {
            From::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
