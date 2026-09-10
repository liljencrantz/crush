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
    short = "Execute a command, optionally recovering from any error it produces.",
    long = "If `body` fails and a `catch` clause is given, the catch clause is invoked",
    long = "instead of letting the error propagate, receiving the error message (a",
    long = "string) as its single unnamed argument.",
    long = "",
    long = "If no catch clause is given, an error in `body` propagates normally, exactly",
    long = "as if `try` weren't there.",
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

    match (cfg.catch_clause, cfg.r#catch.as_str()) {
        (None, "catch") => {
            let env = context
                .scope
                .create_child(&context.scope, ScopeType::Conditional);
            cfg.body
                .eval(context.empty().with_scope(env).with_output(context.output))
        }
        (Some(catch_clause), "catch") => {
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
                Err(e) => {
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
            }
        }
        (_, s) => command_error(format!("Unknown clause `{}`. Did you misspell catch?", s)),
    }
}
