use crate::lang::command::OutputType::{Known};
use crate::lang::errors::{CrushResult, argument_error_legacy};
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{value::Value, value::ValueType};
use signature::signature;
use std::io::{BufReader, Read, Write};
use base64::Engine;

#[signature(
    io.base64.from,
    can_block = true,
    output = Known(ValueType::BinaryInputStream),
    short = "Read a Base64 value and decode it.",
    long = "If no file is specified, use the input, which must be a binary or a string.",
    example = "\"68656c6c6f2c20776f726c6421\" | base64:from",
)]
struct FromSignature {
    #[unnamed()]
    #[description("the files to read from (read from input if no file is specified).")]
    files: Files,
}

pub fn from(context: CommandContext) -> CrushResult<()> {
    let cfg = FromSignature::parse(context.arguments, &context.global_state.printer())?;
    let mut reader = BufReader::new(cfg.files.reader(context.input)?);
    let (pipe_reader, mut writer) = os_pipe::pipe()?;
    context.output.send(Value::BinaryInputStream(Box::from(pipe_reader)))?;
    let mut done = false;
    let mut bufin = [0; 4096];
    let mut bufout = [0; 1024 * 3];
    let codec = base64::prelude::BASE64_STANDARD;

    loop {
        let mut pos = 0;
        loop {
            let read = reader.read(&mut bufin[pos..])?;
            if read == 0 {
                done = true;
                break;
            }
            pos += read;
            if pos == bufin.len() {
                break;
            }
        }
        if pos > 0 {
            let written = codec.decode_slice(&bufin[0..pos], &mut bufout).unwrap();
            writer.write(&bufout[0..written])?;
        }
        if done {
            break;
        }
    }
    Ok(())
}

#[signature(
    io.base64.to,
    can_block = true,
    output = Known(ValueType::BinaryInputStream),
    short = "Write specified binary or string as Base64",
    long = "If no file is specified, produce a binary stream as output.",
    example = "\"hello, world!\" | base64:to",
)]
struct To {
    #[unnamed()]
    file: Files,
}

pub fn to(context: CommandContext) -> CrushResult<()> {
    let cfg = To::parse(context.arguments, &context.global_state.printer())?;
    let mut out = cfg.file.writer(context.output)?;
    let codec = base64::prelude::BASE64_STANDARD;
    match context.input.recv()? {
        Value::String(str) => {
            let input = str.as_bytes();
            let res = codec.encode(input);            
            out.write(res.as_bytes())?;
        }
        Value::Binary(input) => {
            let res = codec.encode(input);
            out.write(res.as_bytes())?;
        }
        Value::BinaryInputStream(mut stream) => {
            let mut buf = [0; 1024 * 3];
            let mut buf2 = [0; 1024 * 4];
            let mut done = false;            
            loop {
                let mut pos = 0;
                loop {
                    let read = stream.read(&mut buf[pos..])?;
                    if read == 0 {
                        done = true;
                        break;
                    }
                    pos += read;
                    if pos == buf.len() {
                        break;
                    }               
                }
                if pos > 0 {
                    let written = codec.encode_slice(&buf[0..pos], &mut buf2).unwrap();
                    out.write(&buf2[0..written])?;
                }
                if done {
                    break;
                }
            }
        }
        v => return argument_error_legacy(format!("Expected a binary stream or a string, encountered `{}`", v.value_type().to_string())),
    }
    Ok(())
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "base64",
        "Base64 conversions",
        Box::new(move |env| {
            FromSignature::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
