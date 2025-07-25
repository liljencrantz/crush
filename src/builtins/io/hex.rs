use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{value::Value, value::ValueType};
use signature::signature;
use std::io::{BufReader, Read, Write};

#[signature(
    io.hex.from,
    can_block = true,
    output = Known(ValueType::BinaryInputStream),
    short = "Read a hexadecimal value and decode it.",
    long = "If no file is specified, use the input, which must be a binary or a string.",
    example = "\"68656c6c6f2c20776f726c6421\" | hex:from",
)]
struct FromSignature {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Files,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg = FromSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut reader = BufReader::new(cfg.files.reader(context.input)?);
    let (pipe_reader, mut writer) = os_pipe::pipe()?;
    context
        .output
        .send(Value::BinaryInputStream(Box::from(pipe_reader)))?;
    let mut bufin = [0; 4096];
    let mut bufout = [0; 2048];
    loop {
        let read = reader.read(&mut bufin)?;
        if read == 0 {
            break;
        }
        hex::decode_to_slice(&bufin[0..read], &mut bufout[0..read / 2])?;
        writer.write(&bufout[0..read / 2])?;
    }
    Ok(())
}

#[signature(
    io.hex.to,
    can_block = true,
    output = Known(ValueType::BinaryInputStream),
    short = "Write specified binary or string as hexadecimal",
    long = "If no file is specified, produce a binary stream as output.",
    example = "\"hello, world!\" | hex:to",
)]
struct To {
    #[unnamed()]
    file: Files,
}

pub fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg = To::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut out = cfg.file.writer(context.output)?;
    match context.input.recv()? {
        Value::String(str) => {
            let input = str.as_bytes();
            let mut buf = vec![0; input.len() * 2];
            hex::encode_to_slice(input, &mut buf)?;
            out.write(&buf)?;
        }
        Value::Binary(input) => {
            let mut buf = vec![0; input.len() * 2];
            hex::encode_to_slice(input, &mut buf)?;
            out.write(&buf)?;
        }
        Value::BinaryInputStream(mut stream) => {
            let mut buf = [0; 2048];
            let mut buf2 = [0; 4096];
            loop {
                let read = stream.read(&mut buf)?;
                if read == 0 {
                    break;
                }
                hex::encode_to_slice(&buf[0..read], &mut buf2[0..read * 2])?;
                out.write(&buf2[0..read * 2])?;
            }
        }
        v => {
            return command_error(
                format!(
                    "`hex:to`: Expected a binary stream or a string, encountered `{}`",
                    v.value_type().to_string()
                ),
            );
        }
    }
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "hex",
        "Hexadecimal conversions",
        Box::new(move |env| {
            FromSignature::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
