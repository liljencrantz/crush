use crate::lang::errors::{CrushResult, argument_error};
use crate::lang::signature::files::Files;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    io.bin.from,
    can_block = true,
    short = "Read specified files (or input) as a binary stream",
    long = "If no file is specified, the input must be either binary or a string which will be converted to a binary using utf-8."
)]
struct From {
    #[unnamed()]
    files: Files,
}

pub fn from(mut context: CommandContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;
    context
        .output
        .send(Value::BinaryInputStream(cfg.files.reader(context.input)?))
}

#[signature(
    io.bin.to,
    can_block = true,
    short = "Write specified iterator of strings to a file (or convert to BinaryStream) separated by newlines"
)]
struct To {
    #[unnamed()]
    file: Files,
}

pub fn to(mut context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.remove_arguments(), &context.source, &context.global_state.printer())?;

    match context.input.recv()? {
        Value::BinaryInputStream(mut input) => {
            let mut out = cfg.file.writer(context.output)?;
            std::io::copy(input.as_mut(), out.as_mut())?;
            Ok(())
        }
        v => argument_error(format!(
            "`bin:to`: Expected input to be a binary stream, got a value of type `{}`",
            v.value_type()
        ), &context.source),
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
