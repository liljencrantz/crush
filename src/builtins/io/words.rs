use crate::lang::errors::CrushResult;
use crate::lang::pipe::TableOutputStream;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{data::table::ColumnType, data::table::Row, value::Value, value::ValueType};
use signature::signature;
use std::io::{BufRead, BufReader};

#[signature(
    io.words.from,
    can_block = true,
    short = "Read input and split on word boundaries.",
    long = "Input can be files or the input pipe, which must be a binary input stream, split on word boundaries, trim away punctuation and discard empty \"words\".",
)]
struct From {
    #[unnamed()]
    #[description("the files to read from (read from input pipe if no file is specified).")]
    files: Files,
}

fn send(output: &TableOutputStream, mut ptr: &str) -> CrushResult<()> {
    ptr = ptr.trim_matches(|c: char| c.is_ascii_punctuation());
    if !ptr.is_empty() {
        output.send(Row::new(vec![Value::from(ptr)]))
    } else {
        Ok(())
    }
}

pub fn from(context: CommandContext) -> CrushResult<()> {
    let output = context
        .output
        .initialize(&[ColumnType::new("word", ValueType::String)])?;
    let cfg = From::parse(context.arguments, &context.global_state.printer())?;

    let mut reader = BufReader::new(cfg.files.reader(context.input)?);

    let mut buf = Vec::<u8>::new();
    let mut token = String::new();
    while reader.read_until(b'\n', &mut buf)? != 0 {
        // this moves the ownership of the read data to s
        // there is no allocation
        let s = String::from_utf8(buf)?;
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
