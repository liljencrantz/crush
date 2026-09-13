use crate::lang::argument::{Argument, ArgumentDefinition, ArgumentEvaluator};
use crate::lang::command::Command;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::{CommandContext, EvalContext};
use crate::lang::state::scope::ScopeType;
use crate::lang::value::{Value, ValueDefinition};
use signature::signature;

#[signature(
    control.r#match,
    short = "Branch on a value against a sequence of cases.",
    long = "The body is a block containing a sequence of arms, where each arm is one of:",
    long = "* `case <value> {<body>}` matches if the subject equals `<value>`.",
    long = "* `any <stream> {<body>}` matches if the subject equals any individual value",
    long = "  produced by reading `<stream>` (e.g. `$(seq 5 10)` or a list).",
    long = "* `is <type> {<body>}` matches if the subject's type is `<type>`.",
    long = "* `default {<body>}` always matches.",
    long = "",
    long = "Arms are tried in order; the first one that matches runs, and the rest are",
    long = "skipped. If nothing matches, `match` errors.",
    example = "match $x {",
    example = "    case 2 {echo \"$x is 2\"}",
    example = "    any $(seq 5 10) {echo \"$x is between 5 and 10\"}",
    example = "    is $string {echo \"$x is a string\"}",
    example = "    default {echo \"I don't know what $x is\"}",
    example = "}",
)]
pub struct Match {
    #[description("the value to match against.")]
    subject: Value,
    #[description("a block containing the match's arms.")]
    body: Command,
}

fn eval_body(value: &Value, context: &CommandContext) -> CrushResult<()> {
    match value {
        Value::Command(body) => {
            let env = context
                .scope
                .create_child(&context.scope, ScopeType::Conditional);
            body.eval(
                context
                    .empty()
                    .with_scope(env)
                    .with_output(context.output.clone()),
            )
        }
        v => command_error(format!(
            "Expected a block, got a value of type `{}`.",
            v.value_type()
        )),
    }
}

fn eval_arm_arguments(
    arguments: &[ArgumentDefinition],
    context: &CommandContext,
) -> CrushResult<Vec<Argument>> {
    let mut eval_context = EvalContext::from(context);
    let (values, _this) = arguments.to_vec().eval(&mut eval_context)?;
    Ok(values)
}

fn r#match(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Match = Match::parse(context.remove_arguments(), &context.global_state.printer())?;

    let jobs = cfg
        .body
        .jobs()
        .ok_or("match's body must be a literal block.")?;

    for job in jobs {
        let commands = job.commands();
        if commands.len() != 1 {
            return command_error(
                "Each match arm must be a single command, e.g. `case 10 {...}`.",
            );
        }
        let invocation = &commands[0];

        // The arm's keyword (`case`/`any`/`is`/`default`) sits in *command position* of
        // its own nested invocation, so it must be read directly off the AST as a bare
        // Identifier rather than evaluated -- evaluating it would try to resolve it as a
        // real variable or command and fail, the same way a real `case` command would if
        // one didn't exist.
        let keyword = match invocation.command() {
            ValueDefinition::Identifier(source) => source.string(),
            v => {
                return command_error(format!(
                    "Each match arm must start with a bareword `case`, `any`, `is` or `default`, found `{}`.",
                    v
                ));
            }
        };

        let args = eval_arm_arguments(invocation.arguments(), &context)?;

        match keyword.as_str() {
            "default" => {
                if args.len() != 1 {
                    return command_error("`default` takes a single block argument.");
                }
                return eval_body(&args[0].value, &context);
            }
            "case" => {
                if args.len() != 2 {
                    return command_error("`case` takes a value and a block argument.");
                }
                if args[0].value == cfg.subject {
                    return eval_body(&args[1].value, &context);
                }
            }
            "any" => {
                if args.len() != 2 {
                    return command_error("`any` takes a stream and a block argument.");
                }
                let mut stream = args[0].value.stream(context.command_handle())?;
                let mut matched = false;
                while let Some(row) = stream.next_row()? {
                    if row.cells().len() != 1 {
                        return command_error(
                            "`any`'s argument must be a single-column stream.",
                        );
                    }
                    if row.cells()[0] == cfg.subject {
                        matched = true;
                        break;
                    }
                }
                if matched {
                    return eval_body(&args[1].value, &context);
                }
            }
            "is" => {
                if args.len() != 2 {
                    return command_error("`is` takes a pattern and a block argument.");
                }
                // Dispatches through the arm's own `__is__` method, the same mechanism
                // `like` and the `=~` operator use -- so `is $string {...}` (a type
                // check) and `is *.txt {...}` (a glob match) both work, as would any
                // other value implementing `__is__`.
                if args[0].value.is(&cfg.subject, &context)? {
                    return eval_body(&args[1].value, &context);
                }
            }
            other => {
                return command_error(format!(
                    "Unknown match arm `{}`. Expected `case`, `any`, `is` or `default`.",
                    other
                ));
            }
        }
    }

    command_error("No match arm matched, and no default arm was given.")
}
