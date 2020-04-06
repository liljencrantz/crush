use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::{
    lang::{
        argument::Argument,
        table::Row,
        value::Value,
    },
    lang::stream::OutputStream,
    lang::errors::{CrushError, argument_error},
};
use std::{
    io::BufReader,
    io::prelude::*,
};

use crate::lang::printer::printer;
use crate::lang::{table::ColumnType, binary::BinaryReader};
use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::stream::ValueReceiver;

pub struct Config {
    separator: char,
    columns: Vec<ColumnType>,
    skip_head: usize,
    trim: Option<char>,
    input: Box<dyn BinaryReader>,
}

fn parse(arguments: Vec<Argument>, input: ValueReceiver) -> CrushResult<Config> {
    let mut separator = ',';
    let mut columns = Vec::new();
    let mut skip_head = 0;
    let mut trim = None;
    let mut files = Vec::new();

    for arg in arguments {
        match &arg.argument_type {
            None => {
                files.push(arg);
            }
            Some(name) => {
                match (name.as_ref(), arg.value) {
                    (_, Value::Type(s)) => columns.push(ColumnType::new(name, s)),

                    ("head", Value::Integer(s)) => skip_head = s as usize,

                    ("separator", Value::String(s)) =>
                        if s.len() == 1 {
                            separator = s.chars().next().unwrap();
                        } else {
                            return argument_error("Separator must be exactly one character long");
                        }

                    ("trim", Value::String(s)) =>
                        if s.len() == 1 {
                            trim = Some(s.chars().next().unwrap());
                        } else {
                            return argument_error("Only one character can be trimmed");
                        }

                    _ => return argument_error(format!("Unknown parameter {}", name).as_str()),
                }
            }
        }
    }

    let reader = match files.len() {
        0 => {
            match input.recv()? {
                Value::BinaryStream(b) => Ok(b),
                Value::Binary(b) => Ok(BinaryReader::vec(&b)),
                _ => argument_error("Expected either a file to read or binary pipe input"),
            }
        }
        _ => BinaryReader::paths(files.files()?),
    }?;

    Ok(Config {
        separator,
        columns,
        skip_head,
        trim,
        input: reader,
    })
}

fn run(cfg: Config, output: OutputStream) -> CrushResult<()> {

    let separator = cfg.separator.clone();
    let trim = cfg.trim.clone();
    let columns = cfg.columns.clone();
    let skip = cfg.skip_head;

    let mut reader = BufReader::new(cfg.input);
    let mut line = String::new();
    let mut skipped = 0usize;
    loop {
        line.clear();
        to_crush_error(reader.read_line(&mut line))?;
        if line.is_empty() {
            break;
        }
        if skipped < skip {
            skipped += 1;
            continue;
        }
        let line_without_newline = &line[0..line.len() - 1];
        let mut split: Vec<&str> = line_without_newline
            .split(separator)
            .map(|s| trim
                .map(|c| s.trim_matches(c))
                .unwrap_or(s))
            .collect();

        if split.len() != columns.len() {
            printer().error("csv: Wrong number of columns in CSV file");
        }

        if let Some(trim) = trim {
            split = split.iter().map(|s| s.trim_matches(trim)).collect();
        }

        match split.iter()
            .zip(columns.iter())
            .map({ |(s, t)| t.cell_type.parse(*s) })
            .collect::<Result<Vec<Value>, CrushError>>() {
            Ok(cells) => {
                let _ = output.send(Row::new(cells));
            }
            Err(err) => {
                printer().crush_error(err);
            }
        }
    }
    Ok(())
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let cfg = parse(context.arguments, context.input)?;
    let output = context.output.initialize(
        cfg.columns.clone())?;
    run(cfg, output)
}
