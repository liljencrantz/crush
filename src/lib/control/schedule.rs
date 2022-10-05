use std::mem::swap;
use crate::lang::errors::{CrushResult, mandate, to_crush_error};
use crate::lang::state::contexts::CommandContext;
use signature::signature;
use chrono::{Duration, Local};
use crate::data::table::ColumnType;
use crate::lang::command::Command;
use crate::lang::data::table::Row;
use crate::lang::pipe::pipe;
use crate::lang::value::ValueType;

#[signature(
schedule,
short = "Schedule recurring events",
long = "If a command is specified, timer will run the command at the specified cadence.\n    Otherwise, if timer is used inside a pipeline, it will read one row of input at the specified cadence and write it out again.\n    Otherwise, timer will simply write an empty row at the specified cadence."
)]
pub struct Schedule {
    #[description("the interval between heartbeats")]
    interval: Duration,
    #[description("the delay for the first heartbeat")]
    initial_delay: Option<Duration>,
    #[description("if heart beat delivery starts blocking, catch up by sending more heartbeats afterwards.")]
    #[default(false)]
    schedule_at_fixed_rate: bool,
    #[description("a command to run")]
    command: Option<Command>,
}

fn schedule(mut context: CommandContext) -> CrushResult<()> {
    let mut cfg: Schedule = Schedule::parse(context.remove_arguments(), &context.global_state.printer())?;

    if let Some(initial_delay) = &cfg.initial_delay {
        std::thread::sleep(to_crush_error(initial_delay.to_std())?);
    }

    let mut cmd = None;
    swap(&mut cmd, &mut cfg.command);
    match cmd {
        None => {
            if context.input.is_pipeline() {
                let mut input = mandate(context.input.recv()?.stream()?, "Expected a stream")?;
                let output = context.output.initialize(input.types())?;
                run(cfg, || { output.send(input.read()?) })
            } else {
                let output = context.output.initialize(&[])?;
                run(cfg, || { output.send(Row::new(vec![])) })
            }
        }
        Some(cmd) => {
            let output = context.output.initialize(&[ColumnType::new("value", ValueType::Any)])?;
            let base_context = context.empty();
            let env = context.scope.clone();
            let (sender, receiver) = pipe();
            run(cfg, || {
                cmd.eval(base_context.clone().with_scope(env.clone()).with_output(sender.clone()))?;
                output.send(Row::new(vec![receiver.recv()?]))
            })
        }
    }
}

fn run(cfg: Schedule, mut f: impl FnMut() -> CrushResult<()>) -> CrushResult<()>{
    if cfg.schedule_at_fixed_rate {
        let mut last_time = Local::now();
        loop {
            f()?;
            last_time = last_time + cfg.interval.clone();
            let next_duration = last_time - Local::now();
            if next_duration > Duration::seconds(0) {
                std::thread::sleep(to_crush_error(next_duration.to_std())?);
            }
        }
    } else {
        loop {
            f()?;
            std::thread::sleep(to_crush_error(cfg.interval.to_std())?);
        }
    }
}
