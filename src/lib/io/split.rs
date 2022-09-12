use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::signature::files::Files;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::pipe::OutputStream;
use crate::lang::{
    data::table::ColumnType, data::table::Row, value::Value,
    value::ValueType,
};
use signature::signature;
use std::io::{BufRead, BufReader};
use crate::lang::state::contexts::CommandContext;

#[signature(
    from,
    can_block = true,
    short = "Read specified files (or input) as a table, split on the specified separator characters.",
)]
struct From {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Files,
    #[description("characters to split on")]
    separator: String,
    #[description("characters to trim from start and end of each token.")]
    trim: Option<String>,
    #[default(false)]
    #[description("allow empty tokens.")]
    allow_empty: bool,
}

fn send(
    output: &OutputStream,
    trim: &Option<String>,
    allow_empty: bool,
    mut ptr: &str,
) -> CrushResult<()> {
    if let Some(t) = trim {
        ptr = ptr.trim_matches(|ch| t.contains(ch));
    }
    if allow_empty || !ptr.is_empty() {
        output.send(Row::new(vec![Value::String(ptr.to_string())]))
    } else {
        Ok(())
    }
}

pub fn from(context: CommandContext) -> CrushResult<()> {
    let output = context
        .output
        .initialize(vec![ColumnType::new("token", ValueType::String)])?;
    let cfg: From = From::parse(context.arguments, &context.global_state.printer())?;

    let mut reader = BufReader::new(cfg.files.reader(context.input)?);

    let mut buf = Vec::<u8>::new();
    let mut token = String::new();
    while to_crush_error(reader.read_until(b'\n', &mut buf))? != 0 {
        // this moves the ownership of the read data to s
        // there is no allocation
        let s = to_crush_error(String::from_utf8(buf))?;
        for c in s.chars() {
            if cfg.separator.contains(c) {
                send(&output, &cfg.trim, cfg.allow_empty, token.as_str())?;
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
    send(&output, &cfg.trim, cfg.allow_empty, token.as_str())?;
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "split",
        "Configurable word splitting I/O",
        Box::new(move |env| {
            From::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
