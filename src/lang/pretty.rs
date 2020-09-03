use crate::lang::data::binary::BinaryReader;
use crate::lang::errors::to_crush_error;
use crate::lang::data::list::ListReader;
use crate::lang::printer::Printer;
use crate::lang::stream::{CrushStream, InputStream, ValueSender, unbounded_channels};
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;
use crate::lang::data::table::Table;
use crate::lang::data::table::TableReader;
use crate::lang::value::Alignment;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::lang::data::r#struct::Struct;
use std::cmp::max;
use std::io::{BufReader, Read};
use std::thread;
use chrono::Duration;
use crate::util::hex::to_hex;

pub fn create_pretty_printer(printer: Printer) -> ValueSender {
    let (o, i) = unbounded_channels();
    let printer_clone = printer.clone();
    printer_clone.handle_error(to_crush_error(
        thread::Builder::new()
            .name("output-formater".to_string())
            .spawn(move || {
                let pp = PrettyPrinter { printer };
                while let Ok(val) = i.recv() {
                    pp.print_value(val);
                }
            }),
    ));
    o
}

pub struct PrettyPrinter {
    printer: Printer,
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
    let hex = c.iter().map(|u| to_hex(*u)).collect::<Vec<String>>().join("");
    let printable = c
        .iter()
        .map(|u| printable(*u))
        .collect::<Vec<String>>()
        .join("");
    return format!("{} {}{}", hex, " ".repeat(64 - hex.len()), printable);
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
        res += "\n<truncated>";
    }
    res
}

fn is_text(buff: &[u8]) -> bool {
    let mut c = 0;
    for v in buff {
        if is_printable(*v) {
            c += 1;
        }
    }
    (c as f64) / (buff.len() as f64) > 0.8
}

impl PrettyPrinter {
    pub fn new(printer: Printer) -> PrettyPrinter {
        PrettyPrinter { printer }
    }

    pub fn print_value(&self, cell: Value) {
        match cell {
            Value::TableStream(mut output) => self.print_stream(&mut output, 0),
            Value::Table(rows) => self.print_stream(&mut TableReader::new(rows), 0),
            Value::BinaryStream(mut b) => self.print_binary(b.as_mut(), 0),
            Value::Empty() => {}
            Value::Struct(data) => {
                self.print_struct(data, 0)
            }
            Value::List(list) => {
                if list.len() < 8 {
                    self.printer.line(list.to_string().as_str())
                } else {
                    self.print_stream(&mut ListReader::new(list, "value"), 0)
                }
            }
            _ => self.printer.line(cell.to_string().as_str()),
        };
    }

    fn print_stream(&self, stream: &mut impl CrushStream, indent: usize) {
        let mut data: Vec<Row> = Vec::new();
        let mut has_table = false;

        for val in stream.types().iter() {
            match val.cell_type {
                ValueType::TableStream(_) => has_table = true,
                ValueType::Table(_) => has_table = true,
                _ => (),
            }
        }

        loop {
            match stream.read_timeout(Duration::milliseconds(100)) {
                Ok(r) => {
                    data.push(r);
                    if data.len() == self.printer.height() - 1 || has_table {
                        self.print_partial(data, stream.types(), indent, has_table);
                        data = Vec::new();
                        data.drain(..);
                    }
                }
                Err(e) => {
                    if e.is_disconnected() {
                        break;
                    } else {
                        self.print_partial(data, stream.types(), indent, has_table);
                        data = Vec::new();
                        data.drain(..);
                    }
                }
            }
        }
        if !data.is_empty() {
            self.print_partial(data, stream.types(), indent, has_table);
        }
    }

    fn calculate_header_width(&self, w: &mut [usize], types: &[ColumnType]) {
        for (idx, val) in types.iter().enumerate() {
            w[idx] = max(w[idx], val.name.len());
        }
    }

    fn calculate_body_width(&self, w: &mut [usize], data: &[Row], col_count: usize) {
        for r in data {
            for (idx, c) in r.cells().iter().enumerate() {
                if idx == col_count {
                    break;
                }
                let l = c.to_string().len();
                w[idx] = max(w[idx], l);
            }
        }
    }

    fn print_header(&self, w: &[usize], types: &[ColumnType], indent: usize) {
        let mut header = " ".repeat(indent * 4);
        let last_idx = types.len() - 1;
        for (idx, val) in types.iter().enumerate() {
            let is_last = idx == last_idx;
            header += val.name.as_ref();
            if !is_last {
                header += &" ".repeat(w[idx] - val.name.len() + 1);
            }
        }
        self.printer.line(header.as_str())
    }

    fn print_row(
        &self,
        w: &[usize],
        mut r: Vec<Value>,
        indent: usize,
        rows: &mut Vec<Table>,
        outputs: &mut Vec<InputStream>,
        binaries: &mut Vec<Box<dyn BinaryReader>>,
        col_count: usize,
    ) {
        let cell_len = r.len();
        let mut row = " ".repeat(indent * 4);
        let last_idx = col_count - 1;
        for (idx, c) in r.drain(..).enumerate() {
            if idx == col_count {
                break;
            }
            let cell = c.to_string();
            let spaces = if idx == cell_len - 1 {
                "".to_string()
            } else {
                " ".repeat(w[idx] - cell.len())
            };
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
        self.printer.line(row.as_str());
    }

    fn print_body(&self, w: &[usize], data: Vec<Row>, indent: usize, last_separate: bool) {
        let col_count = w.len();
        for r in data.into_iter() {
            let mut rows = Vec::new();
            let mut outputs = Vec::new();
            let mut binaries = Vec::new();

            let mut r_vec = r.into_vec();

            if last_separate {
                let last = r_vec.remove(r_vec.len()-1);
                self.print_row(w, r_vec, indent, &mut rows, &mut outputs, &mut binaries, col_count);
                match last {
                    Value::Struct(s) => {
                        self.print_struct(s, indent+1);
                    }
                    _ => panic!("Invalid data"),
                }
            } else {
                self.print_row(w, r_vec, indent, &mut rows, &mut outputs, &mut binaries, col_count);
            }

            for r in rows {
                self.print_stream(&mut TableReader::new(r), indent + 1);
            }
            for mut r in outputs {
                self.print_stream(&mut r, indent + 1);
            }
            for mut r in binaries {
                self.print_binary(r.as_mut(), indent + 1);
            }
        }
    }

    fn print_binary(&self, binary: &mut dyn BinaryReader, _indent: usize) {
        let mut reader = BufReader::new(binary);

        let buff_len = 128 * 1024;
        let mut buff = vec![0; buff_len];
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
                    self.printer.error(e.to_string().as_str());
                    return;
                }
            }
        }
        self.printer
            .line(format_buffer(&buff[0..used], complete).as_str());
    }

    fn print_partial(&self, data: Vec<Row>, types: &[ColumnType], indent: usize, has_table: bool) {
        if data.len() == 0 {
            return;
        }
        if types.len() == 1 && indent == 0 && !has_table {
            self.print_single_column_table(data, types)
        } else {
            let last_separate = types.len() > 0 && indent == 0 && !has_table && types[types.len()-1].cell_type == ValueType::Struct;

            let types = if last_separate {
                &types[0..types.len()-1]
            } else {
                types
            };

            let mut w = vec![0; types.len()];

            self.calculate_header_width(&mut w, types);
            self.calculate_body_width(&mut w, &data, types.len());

            self.print_header(&w, types, indent);
            self.print_body(&w, data, indent, last_separate)
        }
    }

    fn print_struct(&self, s: Struct, indent: usize) {
        let mut data = s.map();
        if data.len() > 0 {
            let max_name_width = data.keys().map(|n| n.len()).max().unwrap();
            for (name, value) in data.drain() {
                if indent * 4 + max_name_width + value.to_string().len() + 2 < self.printer.width() {
                    let mut line = " ".repeat(4 * indent);
                    line.push_str(&name);
                    line.push(':');
                    line.push_str(&" ".repeat(max_name_width - name.len() + 1));
                    line.push_str(&value.to_string());
                    self.printer.line(&line);
                } else {
                    let mut line = " ".repeat(4 * indent);
                    line.push_str(&name);
                    line.push(':');
                    self.printer.line(&line);
                    self.print_struct_value(value, indent+1);
                }
            }
        }
    }

    fn print_struct_value(&self, value: Value, indent: usize) {
        if value.to_string().len() + 4 * indent < self.printer.width() {
            let mut line = " ".repeat(4 * indent);
            line.push_str(&value.to_string());
            self.printer.line(&line);
        } else {
            match value {
                Value::Struct(s) => self.print_struct(s, indent),
                Value::TableStream(mut output) => self.print_stream(&mut output, indent),
                Value::Table(rows) => self.print_stream(&mut TableReader::new(rows), indent),
                Value::BinaryStream(mut b) => self.print_binary(b.as_mut(), indent),
                Value::List(list) => self.print_stream(&mut ListReader::new(list, "value"), indent),
                _ => {
                    let mut line = " ".repeat(4 * indent);
                    line.push_str(&value.to_string());
                    self.printer.line(&line);
                }
            }
        }
    }

    fn print_single_column_table(&self, data: Vec<Row>, types: &[ColumnType]) {
        self.printer.line(&types[0].name);
        let max_width = self.printer.width();
        let mut columns = 1;
        let mut widths = vec![];
        let mut items_per_column;
        let data = data
            .iter()
            .map(|s| s.cells()[0].to_string())
            .collect::<Vec<_>>();

        for cols in (2..50).rev() {
            items_per_column = (data.len() - 1) / cols + 1;
            let ww = data
                .chunks(items_per_column)
                .map(|el| el.iter().map(|v| v.len()).max().unwrap())
                .collect::<Vec<usize>>();
            let tot_width: usize = ww.iter().sum::<usize>() + ww.len() - 1;
            if tot_width <= max_width {
                columns = cols;
                widths = ww;
                break;
            }
        }

        let lines = (data.len() - 1) / columns + 1;
        for start_idx in 0..lines {
            let mut line = "".to_string();
            for (off, idx) in (start_idx..data.len()).step_by(lines).enumerate() {
                line += &data[idx];
                if off + 1 < widths.len() {
                    line += &" ".repeat(widths[off] - data[idx].len() + 1);
                }
            }
            self.printer.line(&line);
        }
    }
}
