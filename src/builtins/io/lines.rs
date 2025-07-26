use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, data_error};
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::binary_input::ToReader;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{data::table::ColumnType, data::table::Row, value::Value, value::ValueType};
use signature::signature;
use std::convert::From;
use std::io::{BufRead, BufReader};

static OUTPUT_TYPE: [ColumnType; 1] = [ColumnType::new("line", ValueType::String)];

#[signature(
    io.lines.from,
    can_block = true,
    output = Known(ValueType::table_input_stream(&OUTPUT_TYPE)),
    short = "Read specified files (or input) as a table with one line of text per row"
)]
struct FromSignature {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Vec<BinaryInput>,

    #[default(false)]
    #[description("do not emit empty lines.")]
    skip_empty_lines: bool,

    #[default(false)]
    #[description("strip whitespace from beginning and end of lines.")]
    strip_whitespace: bool,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(&OUTPUT_TYPE)?;
    let cfg: FromSignature =
        FromSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut reader = BufReader::new(cfg.files.to_reader(context.input)?);
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
        if cfg.strip_whitespace {
            s = s.trim()
        }

        if line.len() > 0 || !cfg.skip_empty_lines {
            output.send(Row::new(vec![Value::from(s)]))?;
        }
        line.clear();
    }
    Ok(())
}

#[signature(
    io.lines.to,
    can_block = true,
    short = "Write specified stream of strings to a file (or convert to BinaryStream) separated by newlines"
)]
struct To {
    #[unnamed()]
    file: Option<Files>,
}

pub fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.remove_arguments(), &context.global_state.printer())?;

    let mut input = context.input.recv()?.stream()?;
    let mut out = crate::lang::signature::files::writer(cfg.file, context.output)?;
    if input.types().len() != 1 || input.types()[0].cell_type != ValueType::String {
        return data_error("Expected an input iterator containing a single column of type string.");
    }
    while let Ok(row) = input.read() {
        match Vec::from(row).remove(0) {
            Value::String(s) => {
                let mut s = s.to_string();
                s.push('\n');
                out.write(s.as_bytes())?;
            }
            _ => {
                return data_error(format!(
                    "Expected the `{}` column to be a string.",
                    input.types()[0].name()
                ));
            }
        }
    }
    Ok(())
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
