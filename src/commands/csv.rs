use crate::commands::CompileContext;
use crate::{
    data::{
        Argument,
        Row,
        ValueType,
        Stream,
        Value,
    },
    stream::{OutputStream, InputStream, unlimited_streams},
    errors::{JobError, argument_error},
};
use std::{
    io::BufReader,
    io::prelude::*,
    fs::File,
    thread,
    path::Path,
};
use either::Either;

extern crate map_in_place;

use map_in_place::MapVecInPlace;
use crate::printer::Printer;
use crate::data::ColumnType;
use crate::errors::JobResult;

pub struct Config {
    separator: char,
    columns: Vec<ColumnType>,
    skip_head: usize,
    trim: Option<char>,
    files: Vec<Box<Path>>,
}

fn parse(arguments: Vec<Argument>, _input: InputStream) -> JobResult<Config> {
    let mut separator = ',';
    let mut columns = Vec::new();
    let mut skip_head = 0;
    let mut trim = None;
    let mut files = Vec::new();

    for arg in arguments {
        match &arg.name {
            None => {
                arg.value.file_expand(&mut files);
            }
            Some(name) => {
                match (name.as_ref(), arg.value) {
                    ("col", Value::Text(s)) => {
                        let split: Vec<&str> = s.split(':').collect();
                        match split.len() {
                            2 => columns.push(ColumnType::named(split[0], ValueType::from(split[1])?)),
                            _ => return Err(argument_error(format!("Expected a column description on the form name:type, got {}", s).as_str())),
                        }
                    }

                    ("head", Value::Integer(s)) => skip_head = s as usize,

                    ("sep", Value::Text(s)) => {
                        if s.len() == 1 {
                            separator = s.chars().next().unwrap();
                        } else {
                            return Err(argument_error("Separator must be exactly one character long"));
                        }
                    }

                    ("trim", Value::Text(s)) => {
                        if s.len() == 1 {
                            trim = Some(s.chars().next().unwrap());
                        } else {
                            return Err(argument_error("Only one character can be trimmed"));
                        }
                    }

                    _ => return Err(argument_error(format!("Unknown parameter {}", name).as_str())),
                }
            }
        }
    }

    Ok(Config {
        separator,
        columns,
        skip_head,
        trim,
        files: files,
    })
}

fn handle(file: Box<Path>, cfg: &Config, output: &OutputStream, printer: &Printer) -> JobResult<()> {
    let (output_stream, input_stream) = unlimited_streams(cfg.columns.clone());
    let out_row = Row {
        cells: vec![
            Value::File(file.clone()),
            Value::Stream(Stream {
                stream: input_stream,
            }),
        ],
    };
    output.send(out_row)?;

    let printer_copy = printer.clone();

    let separator = cfg.separator.clone();
    let trim = cfg.trim.clone();
    let columns = cfg.columns.clone();
    let skip = cfg.skip_head;

    thread::spawn(move || {
        let fff = File::open(file).unwrap();
        let mut reader = BufReader::new(&fff);
        let mut line = String::new();
        let mut skipped = 0usize;
        loop {
            line.clear();
            reader.read_line(&mut line);
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
                printer_copy.error("csv: Wrong number of columns in CSV file");
            }
            if let Some(trim) = trim {
                split = split.map(|s| s.trim_matches(trim));
            }

            match split.iter()
                .zip(columns.iter())
                .map({ |(s, t)| t.cell_type.parse(*s) })
                .collect::<Result<Vec<Value>, JobError>>() {
                Ok(cells) => { output_stream.send(Row { cells }); }
                Err(err) => { printer_copy.job_error(err); }
            }

        }
    });
    return Ok(());
}


pub fn run(config: Config, output: OutputStream, printer: Printer) -> JobResult<()> {
    for file in &config.files {
        handle(file.clone(), &config, &output, &printer)?;
    }
    return Ok(());
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let input = context.input.initialize_stream()?;
    let cfg = parse(context.arguments, input)?;
    let output = context.output.initialize(
        vec![
            ColumnType::named("file", ValueType::File),
            ColumnType::named("data", ValueType::Output(cfg.columns.clone())),
        ])?;
    run(cfg, output, context.printer)
}
