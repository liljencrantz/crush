use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::CrushResult;
use crate::lang::serialization::{deserialize_reader, serialize_writer};
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::binary_input::ToReader;
use crate::lang::signature::files;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use signature::signature;

#[signature(
    io.pup.to,
    can_block = true,
    output = Unknown,
    short = "Serialize to pup format",
    long = "Pup is the native crush serialization format. All Crush types, including",
    long = "lambdas can be serialized to this format.",
    example = "files | pup:to")]
struct To {
    #[unnamed()]
    #[description("destination file to write to. If unspecified, output is returned as a `binary_stream`.")]
    file: Option<Files>,
}

fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.remove_arguments(), &context.global_state.printer())?;
    let command_handle = context.command_handle().clone();
    let mut writer = files::writer(cfg.file, context.output, &command_handle)?;
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
    #[description("source to read from. If unspecified, will read from input, which must be a `string`, `binary` or `binary_stream`.")]
    files: Vec<BinaryInput>,
}

fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.remove_arguments(), &context.global_state.printer())?;
    let command_handle = context.command_handle().clone();
    context.output.send(deserialize_reader(
        &mut cfg.files.to_reader(context.input, &command_handle)?,
        &context.scope,
    )?)
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "pup",
        "Pup I/O",
        None,
        Box::new(move |env| {
            From::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
