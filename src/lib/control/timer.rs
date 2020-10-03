use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::execution_context::CommandContext;
use signature::signature;
use chrono::{Duration, Local};
use crate::lang::data::table::Row;

#[signature(timer, short="Passes a stream of empty rows to act as a reoccurring timer", )]
pub struct Timer {
    #[description("the interval between heartbeats")]
    interval: Duration,
    #[description("the delay for the first heartbeat")]
    initial_delay: Option<Duration>,
    #[description("if heart beat delivery starts blocking, catch up by sending more heartbeats afterwards.")]
    #[default(false)]
    schedule_at_fixed_rate: bool,
}

fn timer(context: CommandContext) -> CrushResult<()> {
    let cfg: Timer = Timer::parse(context.arguments.clone(), &context.global_state.printer())?;
    let output = context.output.initialize(vec![])?;

    if let Some(initial_delay) = &cfg.initial_delay {
        std::thread::sleep(to_crush_error(initial_delay.to_std())?);
    }

    if cfg.schedule_at_fixed_rate {
        let mut last_time = Local::now();
        loop {
            output.send(Row::new(vec![]))?;
            last_time = last_time + cfg.interval.clone();
            let next_duration = last_time - Local::now();
            if next_duration > Duration::seconds(0) {
                std::thread::sleep(to_crush_error(next_duration.to_std())?);
            }
        }
    } else {
        loop {
            output.send(Row::new(vec![]))?;
            std::thread::sleep(to_crush_error(cfg.interval.to_std())?);
        }
    }
    Ok(())
}
