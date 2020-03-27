use crate::lang::stream::{ValueSender, channels, Readable, InputStream};
use crate::lang::printer::printer;
use std::thread;
use crate::lang::table::Table;
use crate::lang::value::Value;
use crate::lang::value::Alignment;
use crate::lang::value::ValueType;
use crate::lang::table::ColumnType;
use crate::lang::table::Row;
use crate::lang::binary::BinaryReader;
use crate::lang::table::TableReader;
use std::cmp::max;
use std::io::{BufReader, BufRead};

pub fn spawn_print_thread() -> ValueSender {
    let (o, i) = channels();
    let _ = thread::Builder::new()
        .name("output-formater".to_string())
        .spawn(move || {
            match i.recv() {
                Ok(val) => print_value( val),
                Err(_) => {}
            }
        });
    o
}

pub fn print_value(cell: Value) {
    match cell {
        Value::TableStream(mut output) => print(&mut output),
        Value::Table(rows) => print(&mut TableReader::new(rows)),
        Value::BinaryStream(mut b) => print_binary(b.as_mut(), 0),
        _ => printer().line(cell.to_string().as_str()),
    };
}

fn print(stream: &mut impl Readable) {
    print_internal(stream, 0);
}

fn print_internal(stream: &mut impl Readable, indent: usize) {
    let mut data: Vec<Row> = Vec::new();
    let mut has_name = false;
    let mut has_table = false;

    for val in stream.types().iter() {
        match val.cell_type {
            ValueType::TableStream(_) => has_table = true,
            ValueType::Table(_) => has_table = true,
            _ => (),
        }
        has_name = true;
    }
    loop {
        match stream.read() {
            Ok(r) => {
                data.push(r)
            }
            Err(_) => break,
        }
        if data.len() == 49 || has_table {
            print_partial( data, stream.types(), has_name, indent);
            data = Vec::new();
            data.drain(..);
        }
    }
    if !data.is_empty() {
        print_partial(data, stream.types(), has_name, indent);
    }
}

fn calculate_header_width(w: &mut Vec<usize>, types: &Vec<ColumnType>, has_name: bool) {
    if has_name {
        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.name.len());
        }
    }
}

fn calculate_body_width(w: &mut Vec<usize>, data: &Vec<Row>, col_count: usize) {
    for r in data {
        assert_eq!(col_count, r.cells().len());
        for (idx, c) in r.cells().iter().enumerate() {
            let l = c.to_string().len();
            w[idx] = max(w[idx], l);
        }
    }
}

fn print_header(w: &Vec<usize>, types: &Vec<ColumnType>, has_name: bool, indent: usize) {
    if has_name {
        let mut header = " ".repeat(indent * 4);
        let last_idx = types.len() - 1;
        for (idx, val) in types.iter().enumerate() {
            let is_last = idx == last_idx;
            header += val.name.as_ref();
            if !is_last {
                header += &" ".repeat(w[idx] - val.name.len() + 1);
            }
        }
        printer().line(header.as_str())
    }
}

fn print_row(
    w: &Vec<usize>,
    r: Row,
    indent: usize,
    rows: &mut Vec<Table>,
    outputs: &mut Vec<InputStream>,
    binaries: &mut Vec<Box<dyn BinaryReader>>) {
    let cell_len = r.len();
    let mut row = " ".repeat(indent * 4);
    let last_idx = r.len() - 1;
    for (idx, c) in r.into_vec().drain(..).enumerate() {
        let cell = c.to_string();
        let spaces = if idx == cell_len - 1 { "".to_string() } else { " ".repeat(w[idx] - cell.len()) };
        let is_last = idx == last_idx;
        match c.alignment() {
            Alignment::Right => {
                row += spaces.as_str();
                row += cell.as_str();
                if !is_last {
                    row += " "
                }
            }
            _ => {
                row += cell.as_str();
                if !is_last {
                    row += spaces.as_str();
                    row += " "
                }
            }
        }

        match c {
            Value::Table(r) => rows.push(r),
            Value::TableStream(o) => outputs.push(o),
            Value::BinaryStream(b) => binaries.push(b),
            _ => {}
        }
    }
    printer().line(row.as_str());
}

fn print_body(w: &Vec<usize>, data: Vec<Row>, indent: usize) {
    for r in data.into_iter() {
        let mut rows = Vec::new();
        let mut outputs = Vec::new();
        let mut binaries = Vec::new();
        print_row( w, r, indent, &mut rows, &mut outputs, &mut binaries);
        for r in rows {
            print_internal(&mut TableReader::new(r), indent + 1);
        }
        for mut r in outputs {
            print_internal(&mut r, indent + 1);
        }
        for mut r in binaries {
            print_binary( r.as_mut(), indent + 1);
        }
    }
}

fn print_binary(binary: &mut dyn BinaryReader, _indent: usize) {
    let mut reader = BufReader::new(binary);

    let mut line = String::new();
    loop {
        line.clear();
        let len = reader.read_line(&mut line).unwrap();
        if len == 0 {
            break;
        }
        let msg = if line.ends_with('\n') { &line[0..line.len() - 1] } else { line.as_str() };
        printer().line(msg);
    }
}

fn print_partial(data: Vec<Row>, types: &Vec<ColumnType>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    calculate_header_width(&mut w, types, has_name);
    calculate_body_width(&mut w, &data, types.len());

    print_header(&w, types, has_name, indent);
    print_body(&w, data, indent)
}
