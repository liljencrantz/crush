use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::{argument_error, to_crush_error, CrushResult};
use crate::lang::files::Files;
use crate::lang::scope::ScopeLoader;
use crate::lang::{execution_context::ExecutionContext, value::Value};
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

pub fn from(context: ExecutionContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.arguments, &context.printer)?;
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

pub fn to(context: ExecutionContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.arguments, &context.printer)?;

    match context.input.recv()? {
        Value::BinaryStream(mut input) => {
            let mut out = cfg.file.writer(context.output)?;
            to_crush_error(std::io::copy(input.as_mut(), out.as_mut()))?;
            Ok(())
        }
        _ => argument_error("Expected a binary stream"),
    }
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_lazy_namespace(
        "bin",
        Box::new(move |env| {
            From::declare(env)?;
            To::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
