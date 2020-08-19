use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::{argument_error, data_error, to_crush_error, CrushResult};
use crate::lang::files::Files;
use crate::lang::scope::ScopeLoader;
use crate::lang::{
    execution_context::CommandContext, table::ColumnType, table::Row, value::Value,
    value::ValueType,
};
use signature::signature;
use std::io::{BufRead, BufReader};

#[signature(
    from,
    can_block = true,
    short = "Read specified files (or input) as a table with one line of text per row"
)]
struct From {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Files,
}

pub fn from(context: CommandContext) -> CrushResult<()> {
    let output = context
        .output
        .initialize(vec![ColumnType::new("line", ValueType::String)])?;
    let cfg: From = From::parse(context.arguments, &context.printer)?;
    let mut reader = BufReader::new(cfg.files.reader(context.input)?);
    let mut line = String::new();

    loop {
        to_crush_error(reader.read_line(&mut line))?;
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
        output.send(Row::new(vec![Value::string(s)]))?;
        line.clear();
    }
    Ok(())
}

#[signature(
    to,
    can_block = true,
    short = "Write specified iterator of strings to a file (or convert to BinaryStream) separated by newlines"
)]
struct To {
    #[unnamed()]
    file: Files,
}

pub fn to(context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.arguments, &context.printer)?;

    match context.input.recv()?.stream() {
        Some(mut input) => {
            let mut out = cfg.file.writer(context.output)?;
            if input.types().len() != 1 || input.types()[0].cell_type != ValueType::String {
                return data_error(
                    "Expected an input iterator containing a single column of type string",
                );
            }
            while let Ok(row) = input.read() {
                match row.into_vec().remove(0) {
                    Value::String(mut s) => {
                        s.push('\n');
                        to_crush_error(out.write(s.as_bytes()))?;
                    }
                    _ => {
                        return data_error("Expected a string");
                    }
                }
            }
            Ok(())
        }
        None => argument_error("Expected a stream"),
    }
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_lazy_namespace(
        "lines",
        Box::new(move |env| {
            From::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
