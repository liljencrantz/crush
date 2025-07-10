use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use chrono::{Duration, Local};
use signature::signature;

#[signature(
    control.timeit,
    short = "Execute a command many times and estimate the execution time.",
    example = "timeit {files|sort size}",
)]
pub struct TimeIt {
    #[description("the command to time.")]
    it: Command,

    #[description(
        "the number of runs in each repeat. If unspecified, timeit will repeat enough times for each batch to take roughly 0.4 seconds."
    )]
    number: Option<usize>,
    
    #[description("repeat count. The average speed in the fastest repeat will be returned.")]
    #[default(5usize)]
    repeat: usize,
}

pub fn time_run(it: &Command, context: &CommandContext) -> CrushResult<Duration> {
    let (sender, reciever) = pipe();

    let c = context.spawn("output consumer", move || {
        let res = reciever.recv()?;
        if let Ok(Some(mut stream)) = res.stream() {
            while let Ok(_) = stream.read() {}
        }
        Ok(())
    })?;

    let start_time = Local::now();
    it.eval(context.clone().with_args(vec![], None).with_output(sender))?;
    context
        .global_state
        .threads()
        .join_one(c, context.global_state.printer());
    let end_time = Local::now();
    Ok(end_time - start_time)
}

fn repeatn(it: &Command, context: &CommandContext, n: usize) -> CrushResult<Duration> {
    let mut times = Vec::new();
    for _ in 0..n {
        times.push(time_run(it, context)?);
    }
    let sum: Duration = times.iter().fold(Duration::seconds(0), |a, b| a + *b);
    Ok(sum / (times.len() as i32))
}

fn timeit(mut context: CommandContext) -> CrushResult<()> {
    let cfg: TimeIt = TimeIt::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.clone();
    let mut repeat_times = Vec::new();

    let number = match cfg.number {
        Some(n) => n,
        None => {
            let mut n = 1usize;
            while repeatn(&cfg.it, &context, n)? * (n as i32) < Duration::milliseconds(400) {
                n *= 2;
            }
            n
        }
    };
    for _ in 0..cfg.repeat {
        repeat_times.push(repeatn(&cfg.it, &context, number)?);
    }
    let tm = repeat_times
        .into_iter()
        .min()
        .ok_or("Failed to run command")?;

    output.send(Value::Duration(tm))
}
