use crate::stream::{OutputStream, InputStream, unlimited_streams};
use crate::cell::{Argument, CellType, Row, CellDataType, Output, Cell};
use crate::commands::{Call, Exec, to_runtime_error};
use crate::errors::{JobError, argument_error, error};
use crate::glob::glob_files;
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std::thread;
use std::path::Path;
use crate::commands::command_util::find_field;
use crate::state::get_cwd;
use either::Either;
use std::thread::JoinHandle;
use regex::Regex;

#[derive(Clone)]
struct Config {
    separator: char,
    columns: Vec<CellType>,
    skip_head: usize,
    trim: Option<char>,
    files: Either<Vec<Box<Path>>, usize>,
}

fn parse(input_type: &Vec<CellType>, arguments: &Vec<Argument>) -> Result<Config, JobError> {
    let mut separator = ',';
    let mut columns = Vec::new();
    let mut skip_head = 0;
    let mut trim = None;
    let mut files = Vec::new();

    files.push(Box::from(Path::new("example_data/age.csv")));
    columns.push(CellType { name: "name".to_string(), cell_type: CellDataType::Text });
    columns.push(CellType { name: "age".to_string(), cell_type: CellDataType::Integer });

    for arg in arguments {}

    Ok(Config {
        separator,
        columns,
        skip_head,
        trim,
        files: Either::Left(files),
    })
}

fn convert(s: &str, t: CellDataType) -> Result<Cell, JobError> {
    match t {
        CellDataType::Text => Ok(Cell::Text(s.to_string())),
        CellDataType::Integer => Ok(Cell::Integer(s.parse::<i128>().unwrap())),
        CellDataType::Field => Ok(Cell::Field(s.to_string())),
        CellDataType::Glob => Ok(Cell::Glob(s.to_string())),
        CellDataType::Regex => Ok(Cell::Regex(s.to_string(), Regex::new(s).unwrap())),
        CellDataType::File => Ok(Cell::Text(s.to_string())),
        _ => panic!("AAAA"),
    }
}


fn handle(file: Box<Path>, cfg: &Config, output: &mut OutputStream) -> Result<(), JobError> {
    let (output_stream, input_stream) = unlimited_streams();
    let cfg_copy = cfg.clone();

    let out_row = Row {
        cells: vec![
            Cell::File(file.clone()),
            Cell::Output(Output {
                types: cfg.columns.clone(),
                stream: input_stream,
            }),
        ],
    };
    output.send(out_row)?;

    let cfg_clone = cfg.clone();

    thread::spawn(move || {
        let fff = File::open(file).unwrap();
        let mut reader = BufReader::new(&fff);
        let mut line = String::new();
        loop {
            reader.read_line(&mut line);
            if line.is_empty() {
                break;
            }
            let line_without_newline = &line[0..line.len() - 1];
            let split: Vec<&str> = line_without_newline
                .split(cfg_clone.separator)
                .map(|s| cfg_clone.trim
                    .map(|c| s.trim_matches(c))
                    .unwrap_or(s))
                .collect();
            if (split.len() != cfg_clone.columns.len()) {
                panic!("Wrong number of columns in CSV file");
//                return Err(error("Wrong number of columns in CSV file"))
            }
            let cells: Result<Vec<Cell>, JobError> = split.iter()
                .zip(cfg_clone.columns.iter())
                .map({ |(s, t)| convert(*s, t.cell_type.clone()) }).collect();

            output_stream.send(Row { cells: cells.unwrap() });
            line.clear();
        }
    });
    return Ok(());
}


fn run(
    input_type: Vec<CellType>,
    mut arguments: Vec<Argument>,
    input: InputStream,
    mut output: OutputStream) -> Result<(), JobError> {
    let cfg = parse(&input_type, &arguments)?;
    match &cfg.files {
        Either::Left(files) => {
            for file in files {
                handle(file.clone(), &cfg, &mut output)?;
            }
        }

        Either::Right(idx) => {}
    }
    return Ok(());
}

pub fn csv(input_type: Vec<CellType>, arguments: Vec<Argument>) -> Result<Call, JobError> {
    let cfg = parse(&input_type, &arguments)?;

    return Ok(Call {
        name: String::from("lines"),
        input_type,
        arguments,
        output_type: cfg.columns.clone(),
        exec: Exec::Run(run),
    });
}
