use crate::{
    lang::Argument,
    lang::Value,
};
use crate::printer::Printer;
use crate::scope::Scope;
use crate::lang::{Stream, RowsReader, ListReader, Struct, DictReader, CrushCommand};
use crate::errors::{argument_error, CrushResult};
use crate::lang::Closure;
use crate::lang::ExecutionContext;
use crate::stream::{empty_channel, Readable};
use crate::stream_printer::spawn_print_thread;

pub struct Config {
    body: Closure,
    env: Scope,
    printer: Printer,
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
                                .map(|(c, t)| {
                                    match &t.name {
                                        None => Argument::unnamed(c),
                                        Some(name) => Argument::named(name.as_ref(), c),
                                    }
                                })
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
                    output: spawn_print_thread(&config.printer),
                    arguments,
                    env: env.clone(),
                    printer: config.printer.clone(),
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

    if let Value::Closure(body) = context.arguments.remove(1).value {
        let iter = context.arguments.remove(0);
        let t = iter.value.value_type();
        let name = iter.name.clone();
        match (iter.name.as_deref(), iter.value) {
            (_, Value::Stream(o)) => {
                run(Config {
                    body,
                    env: context.env,
                    printer: context.printer,
                    name: name,
                }, o.stream)
            }
            (_, Value::Rows(r)) => {
                run(Config {
                    body,
                    env: context.env,
                    printer: context.printer,
                    name: name,
                }, RowsReader::new(r))
            }
            (Some(name), Value::List(l)) => {
                run(Config {
                    body,
                    env: context.env,
                    printer: context.printer,
                    name: None,
                }, ListReader::new(l, name))
            }
            (_, Value::Dict(l)) => {
                run(Config {
                    body,
                    env: context.env,
                    printer: context.printer,
                    name: name,
                }, DictReader::new(l))
            }
            _ => argument_error(format!("Can not iterate over value of type {}", t.to_string()).as_str()),
        }
    } else {
        argument_error("Body must be a closure")
    }
}
