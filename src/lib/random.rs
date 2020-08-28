use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::CrushResult;
use crate::lang::execution_context::CommandContext;
use crate::lang::data::scope::Scope;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    float,
    can_block = false,
    short = "generate a random floating point number between 0 (inclusive) and 1 (exclusive)"
)]
struct Float {
    #[default(1.0)]
    #[description("upper bound.")]
    to: f64,
}

fn float(context: CommandContext) -> CrushResult<()> {
    let cfg: Float = Float::parse(context.arguments, &context.printer)?;
    context
        .output
        .send(Value::Float(rand::random::<f64>() * cfg.to))?;
    Ok(())
}

#[signature(
    integer,
    can_block = false,
    short = "generate a random integer between 0 and 1 (or some other specified number)"
)]
struct Integer {
    #[default(2)]
    #[description("upper bound (exclusive).")]
    to: i128,
}

fn integer(context: CommandContext) -> CrushResult<()> {
    let cfg: Integer = Integer::parse(context.arguments, &context.printer)?;
    let n = rand::random::<f64>() * (cfg.to as f64);
    context.output.send(Value::Integer(n as i128))?;
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "random",
        Box::new(move |env| {
            Float::declare(env)?;
            Integer::declare(env)?;
            Ok(())
        }),
    )?;
    Ok(())
}
