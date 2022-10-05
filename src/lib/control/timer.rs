use chrono::{Duration, Local};
use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use crate::lang::pipe::pipe;
use crate::lang::value::Value;

#[signature(
timer,
short = "Execute a command and return the execution time.",
example = "timer {files|sort size}"
)]
pub struct Timer {
    #[description("the command to time.")]
    it: Command,
}

fn timer(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Timer = Timer::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.output.clone();
    let mut times = Vec::new();

    for _ in 0..cfg.repeats {
        let start_time = Local::now();
        let (sender, reciever) = pipe();
        cfg.it.eval(context.clone().with_args(vec![], None).with_output(sender));
        let res = reciever.recv()?;
        if let (Ok(Some(mut stream))) = res.stream() {
            while let Ok(_) = stream.read() {}
        }
        let end_time = Local::now();
        times.push(end_time - start_time);
    }
    let sum: Duration = times.iter().fold(Duration::seconds(0), |a, b| {a+*b});
    let avg = sum / (times.len() as i32);

    output.send(Value::Duration(avg))
}
