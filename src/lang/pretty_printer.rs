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
use std::io::{BufReader, Read};

pub fn spawn_print_thread() -> ValueSender {
    let (o, i) = channels();
    let _ = thread::Builder::new()
        .name("output-formater".to_string())
        .spawn(move || {
            match i.recv() {
                Ok(val) => print_value(val),
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
            print_partial(data, stream.types(), has_name, indent);
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
                    row += " ";
                }
            }
            _ => {
                row += cell.as_str();
                if !is_last {
                    row += spaces.as_str();
                    row += " ";
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
        print_row(w, r, indent, &mut rows, &mut outputs, &mut binaries);
        for r in rows {
            print_internal(&mut TableReader::new(r), indent + 1);
        }
        for mut r in outputs {
            print_internal(&mut r, indent + 1);
        }
        for mut r in binaries {
            print_binary(r.as_mut(), indent + 1);
        }
    }
}

fn hex(v: u8) -> String {
    let arr = vec!["0", "1", "2", "3", "4", "5", "6", "7", "8", "9", "a", "b", "c", "d", "e", "f"];
    format!("{}{}", arr[(v >> 4) as usize], arr[(v & 15) as usize])
}

fn is_printable(v: u8) -> bool {
    v >= 0x20 && v <= 0x7e
}

fn printable(v: u8) -> String {
    if is_printable(v) {
        (v as char).to_string()
    } else {
        " ".to_string()
    }
}

fn format_binary_chunk(c: &[u8]) -> String {
    let hex = c.iter().map(|u| hex(*u)).collect::<Vec<String>>().join("");
    let printable = c.iter().map(|u| printable(*u)).collect::<Vec<String>>().join("");
    return format!("{} {}{}", hex, " ".repeat(64 - hex.len()), printable);
}

fn is_text(buff: &[u8]) -> bool {
    let mut c = 0;
    for v in buff {
        if is_printable(*v) {
            c += 1;
        }
    }
    return (c as f64) / (buff.len() as f64) > 0.8;
}

fn print_binary(binary: &mut dyn BinaryReader, _indent: usize) {
    let mut reader = BufReader::new(binary);

    let buff_len = 128 * 1024;
    let mut buff = vec![
        0; buff_len
    ];
    let mut complete = false;

    let mut used = 0;
    loop {
        match reader.read(&mut buff[used..buff_len]) {
            Ok(len) => {
                if len == 0 {
                    complete = true;
                    break;
                }
                used += len;
                if used == buff.len() {
                    break;
                }
            }
            Err(e) => {
                printer().error(e.to_string().as_str());
                return;
            }
        }
    }
    printer().line(format_buffer(&buff[0..used], complete).as_str());
}

pub fn format_buffer(buff: &[u8], complete: bool) -> String {
    let s = String::from_utf8(buff.to_vec());

    let mut res = if s.is_ok() && is_text(&buff) {
        s.unwrap()
    } else {
        let mut ss = String::new();
        let chunk_len = 32;
        for chunk in buff.chunks(chunk_len) {
            ss += "\n";
            ss += format_binary_chunk(chunk).as_str();
        }
        ss
    };

    if !complete {
        res +="\n<truncated>";
    }
    res
}

fn print_partial(data: Vec<Row>, types: &Vec<ColumnType>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    calculate_header_width(&mut w, types, has_name);
    calculate_body_width(&mut w, &data, types.len());

    print_header(&w, types, has_name, indent);
    print_body(&w, data, indent)
}
