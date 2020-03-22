use crate::{
    lang::argument::Argument,
    lang::value::Value,
};
use crate::lang::scope::Scope;
use crate::lang::{table::TableReader, list::ListReader, r#struct::Struct, dict::DictReader, command::CrushCommand};
use crate::lang::errors::{argument_error, CrushResult};
use crate::lang::command::Closure;
use crate::lang::command::ExecutionContext;
use crate::lang::stream::{empty_channel, Readable};
use crate::lang::stream_printer::spawn_print_thread;

pub struct Config {
    body: Box<dyn CrushCommand>,
    env: Scope,
    name: Option<Box<str>>,
}

pub fn run(mut config: Config, mut input: impl Readable) -> CrushResult<()> {
    let env = config.env.create_child(&config.env, true);
    loop {
        match input.read() {
            Ok(mut line) => {
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
                });
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

    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }

    if let Value::Command(body) = context.arguments.remove(1).value {
        let iter = context.arguments.remove(0);
        let t = iter.value.value_type();
        let name = iter.name.clone();
        match (iter.name.as_deref(), iter.value) {
            (_, Value::TableStream(o)) => {
                run(Config {
                    body,
                    env: context.env,
                    name: name,
                }, o)
            }
            (_, Value::Table(r)) => {
                run(Config {
                    body,
                    env: context.env,
                    name: name,
                }, TableReader::new(r))
            }
            (Some(name), Value::List(l)) => {
                run(Config {
                    body,
                    env: context.env,
                    name: None,
                }, ListReader::new(l, name))
            }
            (_, Value::Dict(l)) => {
                run(Config {
                    body,
                    env: context.env,
                    name: name,
                }, DictReader::new(l))
            }
            _ => argument_error(format!("Can not iterate over value of type {}", t.to_string()).as_str()),
        }
    } else {
        argument_error("Body must be a closure")
    }
}
