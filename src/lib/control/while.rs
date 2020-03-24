use crate::lang::value::Value;
use crate::lang::scope::Scope;
use crate::lang::command::CrushCommand;
use crate::lang::errors::{argument_error, CrushResult, data_error};
use crate::lang::command::ExecutionContext;
use crate::lang::stream::{empty_channel, Readable, channels};
use crate::lang::stream_printer::spawn_print_thread;

pub struct Config {
    condition: Box<dyn CrushCommand>,
    body: Box<dyn CrushCommand>,
    env: Scope,
}

pub fn run(mut config: Config) -> CrushResult<()> {
    let env = config.env.create_child(&config.env, true);
    loop {
        let (sender, receiver) = channels();

        config.condition.invoke(ExecutionContext {
            input: empty_channel(),
            output: sender,
            arguments: Vec::new(),
            env: config.env.clone(),
            this: None,
        });

        match receiver.recv()? {
            Value::Bool(true) => {
                config.body.invoke(ExecutionContext {
                    input: empty_channel(),
                    output: spawn_print_thread(),
                    arguments: Vec::new(),
                    env: env.clone(),
                    this: None,
                });
                if env.is_stopped() {
                    break;
                }
            }
            Value::Bool(false) => break,
            _ => return data_error("While loop condition must be of boolean type"),
        }
    }
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;

    if context.arguments.len() != 2 {
        return argument_error("Expected exactly two arguments");
    }

    match (context.arguments.remove(0).value, context.arguments.remove(0).value) {
        (Value::Command(condition), Value::Command(body)) =>
            run(Config {
                body,
                condition,
                env: context.env,
            }),
        _ => argument_error("While command expects two closures as arguments"),
    }
}
