use crate::lang::argument::Argument;
use crate::lang::command::Command;
use crate::lang::command::OutputType::Unknown;
use crate::lang::data::r#struct::Struct;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::pipe::Stream;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::ScopeType::Loop;
use crate::lang::value::Value;
use signature::signature;

#[signature(
    control.r#for,
    can_block = true,
    short = "Execute a command once for each element in a stream.",
    output = Unknown,
    example = "# Iterate over the processes on the host",
    example = "for i=$(host:procs) {",
    example = "  echo $(\"Iterating over process {}\":format $i:name)",
    example = "}",
    example = "# Print ten messages",
    example = "for i=$(seq to=10) {",
    example = "  echo $(\"Lap #{}\":format $i)",
    example = "}",
)]
pub struct For {
    #[named()]
    iterator: OrderedStringMap<Stream>,
    body: Command,
}

fn r#for(mut context: CommandContext) -> CrushResult<()> {
    if context.arguments.len() != 2 {
        return command_error("Expected two parameters: A stream and a command");
    }
    let source = context.arguments[0].source.clone();
    let mut cfg = For::parse(context.remove_arguments(), context.global_state.printer())?;

    if cfg.iterator.len() != 1 {
        return command_error(format!("Expected exactly one stream to iterate over, got {}", cfg.iterator.len()));
    }

    let (name, mut input) = cfg
        .iterator
        .drain()
        .next()
        .ok_or("Failed to obtain a stream")?;

    while let Ok(line) = input.read() {
        let env = context.scope.create_child(&context.scope, Loop);

        let vvv = if input.types().len() == 1 {
            Vec::from(line).remove(0)
        } else {
            Value::Struct(Struct::from_vec(Vec::from(line), input.types().to_vec()))
        };

        let arguments = vec![Argument::new(Some(name.clone()), vvv, &source)];

        cfg.body.eval(
            context
                .empty()
                .with_scope(env.clone())
                .with_args(arguments, None),
        )?;
        if env.is_stopped() {
            break;
        }
    }
    context.output.empty()
}
