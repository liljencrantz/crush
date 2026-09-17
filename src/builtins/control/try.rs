use crate::lang::argument::Argument;
use crate::lang::command::Command;
use crate::lang::errors::{CrushError, CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeType;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    control.r#try,
    condition = true,
    short = "Execute a command, recovering from any error it produces.",
    long = "If `body` fails, execution of `body` stops at the failing statement. Zero or",
    long = "more `catch <filter>? {...}` clauses may follow `body`, each the literal word",
    long = "`catch`, an optional filter (a string, glob, or regex -- anything implementing",
    long = "`__is__`, same as `like`/`=~`/match's `is` arm), and a block.",
    long = "",
    long = "On failure, clauses are tried in order; the first one whose filter matches the",
    long = "error's `type` field (see below) -- or that has no filter at all -- runs, and",
    long = "the rest are skipped. Its block receives a struct describing the error as its",
    long = "single unnamed argument: `message` (the error text), `type` (the internal error",
    long = "variant's name, e.g. `IOError`, or a custom type set via `throw`), and",
    long = "`command` (the name of the command that failed, if known -- empty otherwise).",
    long = "",
    long = "If no clause's filter matches, the error propagates normally, exactly as if",
    long = "`try` had no catch clauses of its own. With no catch clauses at all, an error",
    long = "is recovered from silently -- execution continues normally with whatever comes",
    long = "after `try`.",
    example = "try {",
    example = "  risky:command",
    example = "} catch {",
    example = "  |$error| echo (\"Recovered: {}\":format($error:message))",
    example = "}",
    example = "try {",
    example = "  risky:command",
    example = "} catch ^(Serde) {",
    example = "  |$e| echo (\"Serialization error: {}\":format($e:message))",
    example = "} catch Dns* {",
    example = "  |$e| echo (\"DNS error: {}\":format($e:message))",
    example = "}",
)]
pub struct Try {
    #[description("the command to attempt.")]
    body: Command,
    #[unnamed()]
    #[description(
        "zero or more `catch <filter>? {...}` clauses: the literal word `catch`, an optional filter pattern, and a block to run if that filter matches the error's `type` (or always, if no filter is given)."
    )]
    catches: Vec<Value>,
}

struct CatchClause {
    filter: Option<Value>,
    body: Command,
}

/// `catches` is a flat list because a command's own argument list has no way to group
/// repeated clauses -- each clause is the literal word "catch", an optional filter
/// value, and a block, one after another. Validated up front (before `body` even
/// runs) so a malformed catch clause errors immediately rather than only once it's
/// actually needed.
fn parse_catches(raw: Vec<Value>) -> CrushResult<Vec<CatchClause>> {
    let mut clauses = Vec::new();
    let mut iter = raw.into_iter();
    while let Some(kw) = iter.next() {
        match &kw {
            Value::String(s) if s.as_ref() == "catch" => {}
            v => {
                return command_error(format!(
                    "Expected the word `catch`, got `{}`. Did you misspell catch?",
                    v
                ));
            }
        }
        match iter.next() {
            None => return command_error("`catch` requires a block."),
            Some(Value::Command(body)) => clauses.push(CatchClause { filter: None, body }),
            Some(filter) => match iter.next() {
                Some(Value::Command(body)) => clauses.push(CatchClause {
                    filter: Some(filter),
                    body,
                }),
                Some(v) => {
                    return command_error(format!(
                        "Expected a block after catch's filter, got `{}`.",
                        v
                    ));
                }
                None => return command_error("`catch` with a filter also requires a block."),
            },
        }
    }
    Ok(clauses)
}

fn eval_catch(
    catch: Command,
    err: &CrushError,
    context: &CommandContext,
) -> CrushResult<()> {
    let catch_env = context
        .scope
        .create_child(&context.scope, ScopeType::Conditional);
    let arguments = vec![Argument::unnamed(Value::from(err), &context.source)];
    catch.eval(
        context
            .empty()
            .with_scope(catch_env)
            .with_output(context.output.clone())
            .with_args(arguments, None),
    )
}

fn r#try(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Try = Try::parse(context.remove_arguments(), &context.global_state.printer())?;
    let clauses = parse_catches(cfg.catches)?;

    let body_env = context
        .scope
        .create_child(&context.scope, ScopeType::Conditional);
    let body_result = cfg.body.eval(
        context
            .empty()
            .with_scope(body_env)
            .with_output(context.output.clone()),
    );

    let err = match body_result {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    if clauses.is_empty() {
        // No catch clauses at all behaves as if a single catch-all one had been given:
        // the error is recovered from silently, and Empty is sent since body never got
        // to send anything of its own.
        return context.output.send(Value::Empty);
    }

    let error_type = Value::from(err.error_type().type_name());
    for clause in clauses {
        let matched = match &clause.filter {
            None => true,
            Some(filter) => filter.is(&error_type, &context)?,
        };
        if matched {
            return eval_catch(clause.body, &err, &context);
        }
    }

    // No clause's filter matched -- propagate the original error, exactly as if this
    // try had no catch clauses that could ever apply to it.
    Err(err)
}
