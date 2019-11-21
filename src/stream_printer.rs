use crate::stream::{ValueSender, channels, InputStream, Readable, RowsReader};
use crate::printer::Printer;
use std::thread;
use crate::data::{Row, ColumnType, ValueType, Alignment, Value, Rows, Stream};
use std::cmp::max;

pub fn spawn_print_thread(printer: &Printer) -> ValueSender {
    let (o, i) = channels();
    let p = printer.clone();
    thread::Builder::new()
        .name("output-formater".to_string())
        .spawn(move || {
            match i.recv() {
                Ok(val) => print_value(&p, val),
                Err(e) => p.job_error(e),
            }
        }
        );
    o
}

fn print_value(printer: &Printer, mut cell: Value) {
    match cell {
        Value::Stream(mut output) => print(printer, &mut output.stream),
        Value::Rows(rows) => print(printer, &mut RowsReader::new(rows)),
        _ => printer.line(cell.to_string().as_str()),
    };
}

fn print(printer: &Printer, stream: &mut impl Readable) {
    print_internal(printer, stream, 0);
}

fn print_internal(printer: &Printer, stream: &mut impl Readable, indent: usize) {
    let mut data: Vec<Row> = Vec::new();
    let mut has_name = false;
    let mut has_table = false;

    for val in stream.get_type().iter() {
        match val.cell_type {
            ValueType::Output(_) => has_table = true,
            ValueType::Rows(_) => has_table = true,
            _ => (),
        }
        if val.name.is_some() {
            has_name = true;
        }
    }
    loop {
        match stream.read() {
            Ok(r) => {
                data.push(r)
            }
            Err(_) => break,
        }
        if data.len() == 49 || has_table {
            print_partial(printer, data, stream.get_type(), has_name, indent);
            data = Vec::new();
            data.drain(..);
        }
    }
    if !data.is_empty() {
        print_partial(printer, data, stream.get_type(), has_name, indent);
    }
}

fn calculate_header_width(w: &mut Vec<usize>, types: &Vec<ColumnType>, has_name: bool) {
    if has_name {
        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.len_or_0());
        }
    }
}

fn calculate_body_width(w: &mut Vec<usize>, data: &Vec<Row>, col_count: usize) {
    for r in data {
        assert_eq!(col_count, r.cells.len());
        for (idx, c) in r.cells.iter().enumerate() {
            let l = c.to_string().len();
            w[idx] = max(w[idx], l);
        }
    }
}

fn print_header(printer: &Printer, w: &Vec<usize>, types: &Vec<ColumnType>, has_name: bool, indent: usize) {
    if has_name {
        let mut header = " ".repeat(indent * 4);
        for (idx, val) in types.iter().enumerate() {
            header += val.val_or_empty();
            header += &" ".repeat(w[idx] - val.len_or_0() + 1);
        }
        printer.line(header.as_str())
    }
}

fn print_row(printer: &Printer, w: &Vec<usize>, mut r: Row, indent: usize, rows: &mut Vec<Rows>, outputs: &mut Vec<Stream>) {
    let cell_len = r.cells.len();
    let mut row = " ".repeat(indent * 4);
    for (idx, c) in r.cells.drain(..).enumerate() {
        let cell = c.to_string();
        let spaces = if idx == cell_len - 1 { "".to_string() } else { " ".repeat(w[idx] - cell.len()) };
        match c.alignment() {
            Alignment::Right => {
                row += spaces.as_str();
                row += cell.as_str();
                row += " "
            }
            _ => {
                row += cell.as_str();
                row += spaces.as_str();
                row += " "
            }
        }

        match c {
            Value::Rows(r) => rows.push(r),
            Value::Stream(o) => outputs.push(o),
            _ => {}
        }
    }
    printer.line(row.as_str());
}

fn print_body(printer: &Printer, w: &Vec<usize>, data: Vec<Row>, indent: usize) {
    for r in data.into_iter() {
        let mut rows = Vec::new();
        let mut outputs = Vec::new();
        print_row(printer, w, r, indent, &mut rows, &mut outputs);
        for r in rows {
            print_internal(printer, &mut RowsReader::new( r), indent + 1);
        }
        for mut r in outputs {
            print_internal(printer, &mut r.stream, indent + 1);
        }
    }
}

fn print_partial(printer: &Printer, data: Vec<Row>, types: &Vec<ColumnType>, has_name: bool, indent: usize) {
    let mut w = vec![0; types.len()];

    calculate_header_width(&mut w, types, has_name);
    calculate_body_width(&mut w, &data, types.len());

    print_header(printer, &w, types, has_name, indent);
    print_body(printer, &w, data, indent)
}
