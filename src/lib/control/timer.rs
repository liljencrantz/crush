use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::execution_context::CommandContext;
use signature::signature;
use chrono::Duration;
use crate::lang::value::{Value, ValueType};
use crate::lang::data::table::{ColumnType, Row};

#[signature(
r#timer,
short = "Passes a stream of empty values to act as a reoccurring timer",
)]
pub struct Timer {
    #[description("the interval between heartbeats")]
    interval: Duration,
    #[description("the number of heartbeats to send. If unspecified, timer will never stop.")]
    count: Option<i128>,
}

fn timer(context: CommandContext) -> CrushResult<()> {
    let cfg: Timer = Timer::parse(context.arguments.clone(), &context.global_state.printer())?;

    let output = context.output.initialize(vec![
        ColumnType::new("beat", ValueType::Empty)
    ])?;

    match cfg.count {
        None => {
            loop {
                output.send(Row::new(vec![Value::Empty()]))?;
                std::thread::sleep(to_crush_error(cfg.interval.to_std())?);
            }
        }
        Some(count) => {
            for _ in 0..count {
                output.send(Row::new(vec![Value::Empty()]))?;
                std::thread::sleep(to_crush_error(cfg.interval.to_std())?);
            }
        }
    }
    Ok(())
}
