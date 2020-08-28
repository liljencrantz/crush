use crate::lang::argument::Argument;
use crate::lang::errors::{mandate, CrushResult};
use crate::lang::execution_context::{ArgumentVector, CommandContext};
use crate::lang::stream::{black_hole, empty_channel};
use crate::lang::value::Value;
use crate::lang::data::r#struct::Struct;

pub fn r#for(mut context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::Empty())?;
    context.arguments.check_len(2)?;

    let body = context.arguments.command(1)?;
    let iter = context.arguments.remove(0);
    let name = iter.argument_type;
    let mut input = mandate(iter.value.stream(), "Expected a stream")?;

    while let Ok(line) = input.read() {
        let env = context.scope.create_child(&context.scope, true);
        let arguments = match &name {
            None => line
                .into_vec()
                .drain(..)
                .zip(input.types().iter())
                .map(|(c, t)| Argument::named(&t.name, c))
                .collect(),
            Some(var_name) => {
                if input.types().len() == 1 {
                    vec![Argument::new(
                        Some(var_name.clone()),
                        line.into_vec().remove(0),
                    )]
                } else {
                    vec![Argument::new(
                        Some(var_name.clone()),
                        Value::Struct(Struct::from_vec(line.into_vec(), input.types().to_vec())),
                    )]
                }
            }
        };
        body.invoke(CommandContext {
            input: empty_channel(),
            output: black_hole(),
            arguments,
            scope: env.clone(),
            this: None,
            printer: context.printer.clone(),
            threads: context.threads.clone(),
        })?;
        if env.is_stopped() {
            break;
        }
    }
    Ok(())
}
