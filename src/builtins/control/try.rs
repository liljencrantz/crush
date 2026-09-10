use crate::lang::argument::Argument;
use crate::lang::command::Command;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeType;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    control.r#try,
    condition = true,
    short = "Execute a command, recovering from any error it produces.",
    long = "If `body` fails, execution of `body` stops at the failing statement, and",
    long = "`catch` (if given) is invoked instead, receiving the error message (a",
    long = "string) as its single unnamed argument. Either way, the error does not",
    long = "propagate past `try` -- execution continues normally with whatever comes",
    long = "after it, the same as if `catch` had been given but was empty.",
    example = "try {",
    example = "  risky:command",
    example = "} catch {",
    example = "  |$error| echo (\"Recovered: {}\":format $error)",
    example = "}",
)]
pub struct Try {
    #[description("the command to attempt.")]
    body: Command,
    #[default("catch")]
    r#catch: String,
    #[description(
        "the command to invoke if `body` fails, receiving the error message as its unnamed argument."
    )]
    catch_clause: Option<Command>,
}

fn r#try(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Try = Try::parse(context.remove_arguments(), &context.global_state.printer())?;

    if cfg.r#catch.as_str() != "catch" {
        return command_error(format!(
            "Unknown clause `{}`. Did you misspell catch?",
            cfg.r#catch
        ));
    }

    let body_env = context
        .scope
        .create_child(&context.scope, ScopeType::Conditional);
    let body_result = cfg.body.eval(
        context
            .empty()
            .with_scope(body_env)
            .with_output(context.output.clone()),
    );
    match body_result {
        Ok(()) => Ok(()),
        Err(e) => match cfg.catch_clause {
            // No catch clause behaves as if an empty one had been given: the error is
            // recovered from silently, and Empty is sent since body never got to send
            // anything of its own.
            None => context.output.send(Value::Empty),
            Some(catch_clause) => {
                let catch_env = context
                    .scope
                    .create_child(&context.scope, ScopeType::Conditional);
                let arguments = vec![Argument::unnamed(
                    Value::from(e.message().as_str()),
                    &context.source,
                )];
                catch_clause.eval(
                    context
                        .empty()
                        .with_scope(catch_env)
                        .with_output(context.output)
                        .with_args(arguments, None),
                )
            }
        },
    }
}
