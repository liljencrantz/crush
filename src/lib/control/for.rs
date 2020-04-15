use crate::lang::argument::Argument;
use crate::lang::value::Value;
use crate::lang::{table::TableReader, list::ListReader, r#struct::Struct, dict::DictReader, command::CrushCommand};
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::stream::{empty_channel, Readable, black_hole};

pub fn run(
    context: ExecutionContext,
    body: Box<dyn CrushCommand>,
    name: Option<Box<str>>,
    mut input: impl Readable,
) -> CrushResult<()> {
    while let Ok(line) = input.read() {
        let env = context.env.create_child(&context.env, true);
        let arguments =
            match &name {
                None => {
                    line.into_vec()
                        .drain(..)
                        .zip(input.types().iter())
                        .map(|(c, t)|
                            Argument::named(&t.name, c)
                        )
                        .collect()
                }
                Some(var_name) => {
                    vec![Argument::new(
                        Some(var_name.clone()),
                        Value::Struct(Struct::from_vec(
                            line.into_vec(),
                            input.types().to_vec(),
                        )))]
                }
            };
        body.invoke(ExecutionContext {
            input: empty_channel(),
            output: black_hole(),
            arguments,
            env: env.clone(),
            this: None,
            printer: context.printer.clone(),
        })?;
        if env.is_stopped() {
            break;
        }
    }
    Ok(())
}

pub fn r#for(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.clone().initialize(vec![])?;
    context.arguments.check_len(2)?;

    let body = context.arguments.command(1)?;
    let iter = context.arguments.remove(0);
    let t = iter.value.value_type();
    let name = iter.argument_type.clone();

    match (iter.argument_type.as_deref(), iter.value) {
        (_, Value::TableStream(o)) =>
            run(context, body, name, o),
        (_, Value::Table(r)) =>
            run(context, body, name, TableReader::new(r)),
        (Some(name), Value::List(l)) =>
            run(context, body, None, ListReader::new(l, name)),
        (_, Value::Dict(l)) =>
            run(context, body, name, DictReader::new(l)),
        _ => argument_error(format!("Can not iterate over value of type {}", t.to_string()).as_str()),
    }
}
