use crate::lang::errors::{argument_error_legacy, to_crush_error, CrushResult};
use crate::lang::files::Files;
use crate::lang::data::scope::ScopeLoader;
use crate::lang::{execution_context::CommandContext, value::Value};
use signature::signature;

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
        .send(Value::BinaryStream(cfg.files.reader(context.input)?))
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
        Value::BinaryStream(mut input) => {
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
        Box::new(move |env| {
            From::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
