use crate::data::table::ColumnType;
use crate::lang::command::Command;
use crate::lang::data::table::Row;
use crate::lang::errors::CrushResult;
use crate::lang::pipe::pipe;
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use chrono::{Duration, Local};
use signature::signature;
use std::mem::swap;

#[signature(
    control.schedule,
    short = "Schedule events at specified time intervals",
    long = "* If a command is specified, timer will run the command at the specified cadence.",
    long = "* If timer is used inside a pipeline, it will read one row of input at the specified cadence and write it out again.",
    long = "* Otherwise, timer will simply write an empty row at the specified cadence.",
    long = "",
    long = "If a command fails, the timer will stop.",
    example = "# Print hello once every second",
    example = "schedule $(duration:of seconds=1) { echo hello }",
    example = "# Show one row of the pipeline output every second",
    example = "files / --recurse | schedule $(duration:of seconds=1)", 
)]
pub struct Schedule {
    #[description("the interval between heartbeats. ")]
    interval: Duration,

    #[description(
        "the delay for the first heartbeat. If no initial delay is specified, the first heartbeat will be sent immediately."
    )]
    initial_delay: Option<Duration>,

    #[description(
        "if heart beat delivery starts blocking, catch up by sending more heartbeats afterwards."
    )]
    #[default(false)]
    schedule_at_fixed_rate: bool,

    #[description("a command to run.")]
    command: Option<Command>,
}

fn schedule(mut context: CommandContext) -> CrushResult<()> {
    let mut cfg: Schedule =
        Schedule::parse(context.remove_arguments(), &context.global_state.printer())?;

    if let Some(initial_delay) = &cfg.initial_delay {
        std::thread::sleep(initial_delay.to_std()?);
    }

    let mut cmd = None;
    swap(&mut cmd, &mut cfg.command);
    match cmd {
        None => {
            if context.input.is_pipeline() {
                let mut input = context.input.recv()?.stream()?.ok_or("`schedule`: Expected a stream")?;
                let output = context.output.initialize(input.types())?;
                run(cfg, || output.send(input.read()?))
            } else {
                let output = context.output.initialize(&[])?;
                run(cfg, || output.send(Row::new(vec![])))
            }
        }
        Some(cmd) => {
            let output = context
                .output
                .initialize(&[ColumnType::new("value", ValueType::Any)])?;
            let base_context = context.empty();
            let env = context.scope.clone();
            let (sender, receiver) = pipe();
            run(cfg, || {
                cmd.eval(
                    base_context
                        .clone()
                        .with_scope(env.clone())
                        .with_output(sender.clone()),
                )?;
                output.send(Row::new(vec![receiver.recv()?]))
            })
        }
    }
}

fn run(cfg: Schedule, mut f: impl FnMut() -> CrushResult<()>) -> CrushResult<()> {
    if cfg.schedule_at_fixed_rate {
        let mut last_time = Local::now();
        loop {
            f()?;
            last_time = last_time + cfg.interval.clone();
            let next_duration = last_time - Local::now();
            if next_duration > Duration::seconds(0) {
                std::thread::sleep(next_duration.to_std()?);
            }
        }
    } else {
        loop {
            f()?;
            std::thread::sleep(cfg.interval.to_std()?);
        }
    }
}
