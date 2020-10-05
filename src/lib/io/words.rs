use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::files::Files;
use crate::lang::data::scope::ScopeLoader;
use crate::lang::pipe::OutputStream;
use crate::lang::{
    execution_context::CommandContext, data::table::ColumnType, data::table::Row, value::Value,
    value::ValueType,
};
use signature::signature;
use std::io::{BufRead, BufReader};

#[signature(
    from,
    can_block = true,
    short = "Read specified files (or input) as a table, split on word boundaries, and trim away punctuation."
)]
struct From {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Files,
}

fn send(output: &OutputStream, mut ptr: &str) -> CrushResult<()> {
    ptr = ptr.trim_matches(|c: char| c.is_ascii_punctuation());
    if !ptr.is_empty() {
        output.send(Row::new(vec![Value::String(ptr.to_string())]))
    } else {
        Ok(())
    }
}

pub fn from(context: CommandContext) -> CrushResult<()> {
    let output = context
        .output
        .initialize(vec![ColumnType::new("word", ValueType::String)])?;
    let cfg: From = From::parse(context.arguments, &context.global_state.printer())?;

    let mut reader = BufReader::new(cfg.files.reader(context.input)?);

    let mut buf = Vec::<u8>::new();
    let mut token = String::new();
    while to_crush_error(reader.read_until(b'\n', &mut buf))? != 0 {
        // this moves the ownership of the read data to s
        // there is no allocation
        let s = to_crush_error(String::from_utf8(buf))?;
        for c in s.chars() {
            if c.is_whitespace() {
                send(&output, token.as_str())?;
                token.clear();
            } else {
                token.push(c);
            }
        }
        // this returns the ownership of the read data to buf
        // there is no allocation
        buf = s.into_bytes();
        buf.clear();
    }
    send(&output, token.as_str())?;
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "words",
        "Word splitting I/O",
        Box::new(move |env| {
            From::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
