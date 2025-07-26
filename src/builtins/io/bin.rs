use crate::lang::errors::{CrushResult, command_error};
use crate::lang::signature::binary_input::BinaryInput;
use crate::lang::signature::binary_input::ToReader;
use crate::lang::signature::files;
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::value::Value;
use signature::signature;
use std::io::Write;

#[signature(
    io.bin.from,
    can_block = true,
    short = "Read specified files (or input) as a binary stream",
    long = "If no file is specified, the input must be either binary or a string which will be converted to a binary using utf-8."
)]
struct From {
    #[unnamed()]
    #[description(
        "source to read from. If unspecified, will read from input, which must be a `string`, `binary` or `binary_stream`."
    )]
    files: Vec<BinaryInput>,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let mut cfg: From = From::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::BinaryInputStream(
        cfg.files.to_reader(context.input)?,
    ))
}

#[signature(
    io.bin.to,
    can_block = true,
    short = "Write specified iterator of strings to a file (or convert to BinaryStream) separated by newlines"
)]
struct To {
    #[unnamed()]
    file: Option<Files>,
}

pub fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.remove_arguments(), &context.global_state.printer())?;

    match context.input.recv()? {
        Value::BinaryInputStream(mut input) => {
            let mut out = files::writer(cfg.file, context.output)?;
            std::io::copy(input.as_mut(), out.as_mut())?;
            Ok(())
        }
        v => command_error(format!(
            "Expected input to be a `binary_stream`, got a value of type `{}`.",
            v.value_type()
        )),
    }
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_namespace(
        "bin",
        "Binary data I/O",
        Box::new(move |env| {
            From::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
