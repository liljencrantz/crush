use crate::stream::{OutputStream, InputStream, unlimited_streams};
use crate::data::{Argument, CellType, Row, CellDataType, Output};
use crate::data::cell::Cell;
use crate::commands::{Call, Exec};
use crate::errors::{JobError, argument_error, error};
use std::io::BufReader;
use std::io::prelude::*;
use std::fs::File;
use std::thread;
use std::path::Path;
use either::Either;

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

    for arg in arguments {
        if arg.name.is_empty() {
            match &arg.cell {
                Cell::File(s) => files.push(s.clone()),
                Cell::Text(s) => files.push(Box::from(Path::new(&s))),
                _ => panic!("Noooo"),
            }
        } else if arg.name.as_str() == "col" {
            match &arg.cell {
                Cell::Text(s) => {
                    let split: Vec<&str> = s.split(':').collect();
                    match split.len() {
                        2 => columns.push(CellType { name: split[0].to_string(), cell_type: CellDataType::from(split[1]) }),
                        _ => panic!("No no no")
                    }
                }
                _ => panic!("Noooo"),
            }
        }
    }

    Ok(Config {
        separator,
        columns,
        skip_head,
        trim,
        files: Either::Left(files),
    })
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
            if split.len() != cfg_clone.columns.len() {
                panic!("Wrong number of columns in CSV file");
//                return Err(error("Wrong number of columns in CSV file"))
            }
            let cells: Result<Vec<Cell>, JobError> = split.iter()
                .zip(cfg_clone.columns.iter())
                .map({ |(s, t)| t.cell_type.parse(*s) }).collect();

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

    let output_type: Vec<CellType> =
        vec![
            CellType { name: "file".to_string(), cell_type: CellDataType::File },
            CellType { name: "data".to_string(), cell_type: CellDataType::Output(cfg.columns.clone()) },
        ];

    return Ok(Call {
        name: String::from("csv"),
        input_type,
        arguments,
        output_type,
        exec: Exec::Run(run),
    });
}
