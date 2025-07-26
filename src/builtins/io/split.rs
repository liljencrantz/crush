use crate::lang::errors::CrushResult;
use crate::lang::pipe::TableOutputStream;
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::binary_input::ToReader;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{data::table::ColumnType, data::table::Row, value::Value, value::ValueType};
use signature::signature;
use std::io::{BufRead, BufReader};

#[signature(
    io.split.from,
    can_block = true,
    short = "Read specified files (or input) as a table, split on the specified separator characters.",
)]
struct From {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Vec<BinaryInput>,
    #[description("characters to split on")]
    separator: String,
    #[description("characters to trim from start and end of each token.")]
    trim: Option<String>,
    #[default(false)]
    #[description("allow empty tokens.")]
    allow_empty: bool,
}

fn send(
    output: &TableOutputStream,
    trim: &Option<String>,
    allow_empty: bool,
    mut ptr: &str,
) -> CrushResult<()> {
    if let Some(t) = trim {
        ptr = ptr.trim_matches(|ch| t.contains(ch));
    }
    if allow_empty || !ptr.is_empty() {
        output.send(Row::new(vec![Value::from(ptr)]))
    } else {
        Ok(())
    }
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let output = context
        .output
        .initialize(&[ColumnType::new("token", ValueType::String)])?;
    let cfg: From = From::parse(context.remove_arguments(), &context.global_state.printer())?;

    let mut reader = BufReader::new(cfg.files.to_reader(context.input)?);

    let mut buf = Vec::<u8>::new();
    let mut token = String::new();
    while reader.read_until(b'\n', &mut buf)? != 0 {
        // this moves the ownership of the read data to s
        // there is no allocation
        let s = String::from_utf8(buf)?;
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
