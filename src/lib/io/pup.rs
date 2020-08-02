use std::io::{BufReader};

use crate::lang::execution_context::ExecutionContext;
use crate::lang::errors::{CrushResult};
use crate::lang::scope::{Scope, ScopeLoader};
use crate::lang::serialization::{serialize_writer, deserialize_reader};
use crate::lang::command::OutputType::{Known, Unknown};
use crate::lang::value::ValueType;

fn to(mut context: ExecutionContext) -> CrushResult<()> {
    let value = context.input.recv()?;
    serialize_writer(&value, &mut context.writer()?)?;
    context.output.empty()
}

fn from(mut context: ExecutionContext) -> CrushResult<()> {
    let mut reader = context.reader()?;
    context.output.send(deserialize_reader(&mut BufReader::new(&mut reader), &context.env )?)
}

pub fn declare(root: &mut ScopeLoader) -> CrushResult<()> {
    root.create_lazy_namespace(
        "pup",
        Box::new(move |env| {
            env.declare_command(
                "from", from, true,
                "pup:from [file:file]", "Parse pup format", Some(
                    r#"    Input can either be a binary stream or a file.

    Examples:

    pup:from serialized.pup"#),
            Unknown)?;

            env.declare_command(
                "to", to, true,
                "pup:to [file:file]", "Serialize to pup format", Some(
                    r#"    Pup is the native crush serialization format. All pup types, including
   lambdas can be serialized to this format.

    Examples:

    ls | pup:to"#),
            Known(ValueType::Empty))?;
            Ok(())
        }))?;
    Ok(())
}
