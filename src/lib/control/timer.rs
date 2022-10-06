use chrono::Local;
use crate::lang::command::Command;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use crate::lang::pipe::pipe;
use crate::lang::value::Value;
use crate::lib::control::timeit::time_run;

#[signature(
timer,
short = "Execute a command once and return the execution time.",
example = "timer {files|sort size}"
)]
pub struct Timer {
    #[description("the command to time.")]
    it: Command,
}

fn timer(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Timer = Timer::parse(context.remove_arguments(), &context.global_state.printer())?;
    context.output.send(Value::Duration(time_run(&cfg.it, &context)?))
}
