use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::{value::Value, value::ValueType};
use base64::Engine;
use base64::engine::GeneralPurpose;
use signature::signature;
use std::io::{BufReader, Read, Write};

#[signature(
    io.base64.from,
    can_block = true,
    output = Known(ValueType::BinaryInputStream),
    short = "Read a Base64 value and decode it.",
    long = "If no file is specified, use the input, which must be a binary or a string.",
    long = "",
    long = "The standard alphabet follows RFC 4648, and uses `a`-`z`, `A`-`Z`, `0`-`9`, `+`, and `/`. The urlsafe alphabet uses `a-`z, `A`-`Z`, `0`-`9`, `-`, and `.`. Both use `=` for padding.",
    example = "Will output hello, world!",
    example = "\"aGVsbG8sIHdvcmxkIQ==\" | base64:from",
)]
struct FromSignature {
    #[unnamed()]
    #[description("the files to read from. Read from input if no file is specified.")]
    files: Files,
    #[allowed("standard", "urlsafe")]
    #[default("standard")]
    #[description("base64 encoding style to use.")]
    alphabet: String,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg = FromSignature::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut reader = BufReader::new(cfg.files.reader(context.input)?);
    let (pipe_reader, mut writer) = os_pipe::pipe()?;
    context
        .output
        .send(Value::BinaryInputStream(Box::from(pipe_reader)))?;
    let mut done = false;
    let mut bufin = [0; 4096];
    let mut bufout = [0; 1024 * 3];
    let codec = codec(&cfg.alphabet)?;

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
    long = "",
    long = "The standard alphabet follows RFC 4648, and uses `a`-`z`, `A`-`Z`, `0`-`9`, `+`, and `/`. The urlsafe alphabet uses `a-`z, `A`-`Z`, `0`-`9`, `-`, and `.`. Both use `=` for padding.",
    example = "Will output aGVsbG8sIHdvcmxkIQ==",
    example = "\"hello, world!\" | base64:to",
)]
struct To {
    #[unnamed()]
    #[description("the file to write to. Write to output if no file is specified.")]
    file: Files,
    #[allowed("standard", "urlsafe")]
    #[default("standard")]
    #[description("base64 encoding style to use.")]
    alphabet: String,
}

fn codec(name: &str) -> CrushResult<GeneralPurpose> {
    match name {
        "standard" => Ok(base64::prelude::BASE64_STANDARD),
        "urlsafe" => Ok(base64::prelude::BASE64_URL_SAFE),
        _ => command_error(format!("Unknown base64 alphabet `{}`", name)),
    }
}

pub fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg = To::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut out = cfg.file.writer(context.output)?;
    let codec = codec(&cfg.alphabet)?;

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
        v => {
            return command_error(
                format!(
                    "`base64:to`: Expected a binary stream or a string, encountered `{}`",
                    v.value_type().to_string()
                )
            );
        }
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
