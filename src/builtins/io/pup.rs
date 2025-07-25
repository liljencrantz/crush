use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::CrushResult;
use crate::lang::serialization::{deserialize_reader, serialize_writer};
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use signature::signature;
use std::io::BufReader;

#[signature(
    io.pup.to,
    can_block = true,
    output = Unknown,
    short = "Serialize to pup format",
    long = "Pup is the native crush serialization format. All Crush types, including",
    long = "lambdas can be serialized to this format.",
    example = "ls | pup:to")]
struct To {
    #[unnamed()]
    file: Files,
}

fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.remove_arguments(), &context.global_state.printer())?;
    let mut writer = cfg.file.writer(context.output)?;
    let value = context.input.recv()?;
    serialize_writer(&value, &mut writer)
}

#[signature(
    io.pup.from,
    can_block = true,
    output = Unknown,
    short = "Parse pup format",
    example = "pup:from serialized.pup")]
struct From {
    #[unnamed()]
    files: Files,
}

fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(deserialize_reader(
        &mut BufReader::new(&mut cfg.files.reader(context.input)?),
        &context.scope,
    )?)
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "pup",
        "Pup I/O",
        Box::new(move |env| {
            From::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
