use crate::{
    lang::argument::Argument,
    lang::value::Value,
};
use crate::lang::scope::Scope;
use crate::lang::{table::TableReader, list::ListReader, r#struct::Struct, dict::DictReader, command::CrushCommand};
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::stream::{empty_channel, Readable};
use crate::lang::pretty_printer::spawn_print_thread;
use crate::util::replace::Replace;

pub struct Config {
    body: Box<dyn CrushCommand>,
    env: Scope,
    name: Option<Box<str>>,
}

pub fn run(config: Config, mut input: impl Readable) -> CrushResult<()> {
    let env = config.env.create_child(&config.env, true);
    loop {
        match input.read() {
            Ok(line) => {
                let arguments =
                    match &config.name {
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
                                    input.types().clone(),
                                )))]
                        }
                    };
                config.body.invoke(ExecutionContext {
                    input: empty_channel(),
                    output: spawn_print_thread(),
                    arguments,
                    env: env.clone(),
                    this: None,
                })?;
                if env.is_stopped() {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    context.arguments.check_len(2)?;

    let body = context.arguments.command(1)?;
    let iter = context.arguments.remove(0);
    let env = context.env;
    let t = iter.value.value_type();
    let name = iter.argument_type.clone();

    match (iter.argument_type.as_deref(), iter.value) {
        (_, Value::TableStream(o)) =>
            run(Config { body, env, name, }, o),
        (_, Value::Table(r)) =>
            run(Config { body, env, name, }, TableReader::new(r)),
        (Some(name), Value::List(l)) =>
            run(Config { body, env, name: None, }, ListReader::new(l, name)),
        (_, Value::Dict(l)) =>
            run(Config { body, env, name }, DictReader::new(l)),
        _ => argument_error(format!("Can not iterate over value of type {}", t.to_string()).as_str()),
    }
}
