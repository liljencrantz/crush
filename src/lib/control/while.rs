use crate::lang::value::Value;
use crate::lang::scope::Scope;
use crate::lang::command::{CrushCommand, ArgumentVector};
use crate::lang::errors::{argument_error, CrushResult, data_error};
use crate::lang::command::ExecutionContext;
use crate::lang::stream::{empty_channel, channels};
use crate::lang::pretty_printer::spawn_print_thread;

pub struct Config {
    condition: Box<dyn CrushCommand>,
    body: Box<dyn CrushCommand>,
    env: Scope,
}

pub fn run(config: Config) -> CrushResult<()> {
    let env = config.env.create_child(&config.env, true);
    loop {
        let (sender, receiver) = channels();

        config.condition.invoke(ExecutionContext {
            input: empty_channel(),
            output: sender,
            arguments: Vec::new(),
            env: config.env.clone(),
            this: None,
        })?;

        match receiver.recv()? {
            Value::Bool(true) => {
                config.body.invoke(ExecutionContext {
                    input: empty_channel(),
                    output: spawn_print_thread(),
                    arguments: Vec::new(),
                    env: env.clone(),
                    this: None,
                })?;
                if env.is_stopped() {
                    break;
                }
            }
            Value::Bool(false) => break,
            _ => return data_error("While loop condition must output value of boolean type"),
        }
    }
    Ok(())
}

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    context.output.initialize(vec![])?;
    context.arguments.check_len(2)?;

    let condition = context.arguments.command(0)?;
    let body = context.arguments.command(1)?;
    run(Config { body, condition, env: context.env, })
}
