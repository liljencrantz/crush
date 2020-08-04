use std::io::BufReader;
use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::CrushResult;
use crate::lang::scope::ScopeLoader;
use crate::lang::serialization::{serialize_writer, deserialize_reader};
use crate::lang::command::OutputType::{Unknown};
use crate::lang::files::Files;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

#[signature(
to,
can_block = true,
output = Unknown,
short = "Serialize to pup format",
long = "Pup is the native crush serialization format. All pup types, including",
long = "lambdas can be serialized to this format.",
example = "ls | pup:to")]
struct To {
    #[unnamed()]
    file: Files,
}

fn to(context: ExecutionContext) -> CrushResult<()> {
    let cfg: To = To::parse(context.arguments, &context.printer)?;
    let mut writer = cfg.file.writer(context.output)?;
    let value = context.input.recv()?;
    serialize_writer(&value, &mut writer)
}

#[signature(
from,
can_block = true,
output = Unknown,
short = "Parse pup format",
example = "pup:from serialized.pup")]
struct From {
    #[unnamed()]
    files: Files,
}

fn from(context: ExecutionContext) -> CrushResult<()> {
    let cfg: From = From::parse(context.arguments, &context.printer)?;
    context.output.send(deserialize_reader(&mut BufReader::new(&mut cfg.files.reader(context.input)?), &context.env )?)
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_lazy_namespace(
        "pup",
        Box::new(move |env| {
            From::declare(env);
            To::declare(env);
            Ok(())
        }))?;
    Ok(())
}
