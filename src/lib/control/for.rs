use crate::lang::argument::Argument;
use crate::lang::errors::{mandate, CrushResult};
use crate::lang::state::contexts::{ArgumentVector, CommandContext};
use crate::lang::value::Value;
use crate::lang::data::r#struct::Struct;
use crate::lang::value::ValueType;
use crate::lang::data::table::ColumnType;
use lazy_static::lazy_static;
use crate::lang::pipe::pipe;

lazy_static! {
    static ref OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("value", ValueType::Any),
    ];
}

pub fn r#for(mut context: CommandContext) -> CrushResult<()> {
    let (sender, receiver) = pipe();

    context.arguments.check_len(2)?;

    let location = context.arguments[0].location;
    let body = context.arguments.command(1)?;
    let iter = context.arguments.remove(0);
    let name = iter.argument_type;
    let mut input = mandate(iter.value.stream()?, "Expected a stream")?;

    while let Ok(line) = input.read() {
        let env = context.scope.create_child(&context.scope, true);
        let arguments = match &name {
            None => Vec::from(line)
                .drain(..)
                .zip(input.types().iter())
                .map(|(c, t)| Argument::named(&t.name, c, location))
                .collect(),
            Some(var_name) => {
                if input.types().len() == 1 {
                    vec![Argument::new(
                        Some(var_name.clone()),
                        Vec::from(line).remove(0),
                        location,
                    )]
                } else {
                    vec![Argument::new(
                        Some(var_name.clone()),
                        Value::Struct(Struct::from_vec(Vec::from(line), input.types().to_vec())),
                        location,
                    )]
                }
            }
        };
        body.eval(context.empty().with_scope(env.clone()).with_args(arguments, None).with_output(sender.clone()))?;
        if env.is_stopped() {
            context.output.send(receiver.recv()?)?;
            break;
        }
        receiver.recv()?;
    }
    Ok(())
}
