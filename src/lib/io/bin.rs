use crate::lang::errors::{argument_error_legacy, CrushResult, to_crush_error};
use crate::lang::signature::files::Files;
use crate::lang::state::scope::ScopeLoader;
use crate::lang::value::Value;
use signature::signature;
use crate::lang::state::contexts::CommandContext;

#[signature(
    from,
    can_block = true,
    short = "Read specified files (or input) as a binary stream"
)]
struct From {
    #[unnamed()]
    files: Files,
}

pub fn from(context: CommandContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.arguments, &context.global_state.printer())?;
    context
        .output
        .send(Value::BinaryInputStream(cfg.files.reader(context.input)?))
}

#[signature(
    to,
    can_block = true,
    short = "Write specified iterator of strings to a file (or convert to BinaryStream) separated by newlines"
)]
struct To {
    #[unnamed()]
    file: Files,
}

pub fn to(context: CommandContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.arguments, &context.global_state.printer())?;

    match context.input.recv()? {
        Value::BinaryInputStream(mut input) => {
            let mut out = cfg.file.writer(context.output)?;
            to_crush_error(std::io::copy(input.as_mut(), out.as_mut()))?;
            Ok(())
        }
        _ => argument_error_legacy("Expected a binary stream"),
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
