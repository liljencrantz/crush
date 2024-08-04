use crate::lang::errors::{argument_error_legacy, CrushResult, data_error};
use crate::lang::signature::files::Files;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{
    data::table::ColumnType, data::table::Row, value::Value,
    value::ValueType,
};
use signature::signature;
use std::io::{BufRead, BufReader};
use std::convert::From;
use crate::lang::state::contexts::CommandContext;

#[signature(
    io.lines.from,
    can_block = true,
    short = "Read specified files (or input) as a table with one line of text per row"
)]

struct FromSignature {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Files,
}

pub fn from(context: CommandContext) -> CrushResult<()> {
    let output = context
        .output
        .initialize(&[ColumnType::new("line", ValueType::String)])?;
    let cfg: FromSignature = FromSignature::parse(context.arguments, &context.global_state.printer())?;
    let mut reader = BufReader::new(cfg.files.reader(context.input)?);
    let mut line = String::new();

    loop {
        reader.read_line(&mut line)?;
        if line.is_empty() {
            break;
        }
        let mut s = if line.ends_with('\n') {
            &line[0..line.len() - 1]
        } else {
            &line[..]
        };
        while s.starts_with('\r') {
            s = &s[1..];
        }
        while s.ends_with('\r') {
            s = &s[0..line.len() - 1];
        }
        output.send(Row::new(vec![Value::from(s)]))?;
        line.clear();
    }
    Ok(())
}

#[signature(
    io.lines.to,
    can_block = true,
    short = "Write specified iterator of strings to a file (or convert to BinaryStream) separated by newlines"
)]
struct To {
    #[unnamed()]
    file: Files,
}

pub fn to(context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.arguments, &context.global_state.printer())?;

    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let mut out = cfg.file.writer(context.output)?;
            if input.types().len() != 1 || input.types()[0].cell_type != ValueType::String {
                return data_error(
                    "Expected an input iterator containing a single column of type string",
                );
            }
            while let Ok(row) = input.read() {
                match Vec::from(row).remove(0) {
                    Value::String(s) => {
                        let mut s = s.to_string();
                        s.push('\n');
                        out.write(s.as_bytes())?;
                    }
                    _ => {
                        return data_error("Expected a string");
                    }
                }
            }
            Ok(())
        }
        None => argument_error_legacy("Expected a stream"),
    }
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "lines",
        "Line based I/O",
        Box::new(move |env| {
            FromSignature::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
