use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::r#struct::Struct;
use crate::lang::scope::Scope;
use crate::lang::value::Value;
use signature::signature;
use sys_info;
use crate::lang::table::Row;
use battery::State;
use chrono::Duration;
use lazy_static::lazy_static;
use crate::lang::table::ColumnType;
use crate::lang::value::ValueType;
use crate::lang::command::OutputType::Known;

lazy_static! {
    static ref BATTERY_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("vendor", ValueType::String),
        ColumnType::new("model", ValueType::String),
        ColumnType::new("cycle_count", ValueType::Integer),
        ColumnType::new("health", ValueType::Integer),
        ColumnType::new("state", ValueType::String),
        ColumnType::new("charge", ValueType::Integer),
        ColumnType::new("time_to_full", ValueType::Duration),
        ColumnType::new("time_to_empty", ValueType::Duration),
    ];
}

#[signature(
battery,
can_block = true,
output = Known(ValueType::TableStream(BATTERY_OUTPUT_TYPE.clone())),
short = "List all batteries in the system and their status")]
struct Battery {}

fn state_name(state: battery::State) -> String {
    match state {
        State::Unknown => "Unknown",
        State::Charging => "Charging",
        State::Discharging => "Discharging",
        State::Empty => "Empty",
        State::Full => "Full",
        _ => "Unknown",
    }.to_string()
}

fn time_to_duration(tm: Option<battery::units::Time>) -> Duration {
    tm.map(|t| Duration::seconds(t.value as i64)).unwrap_or(Duration::seconds(0))
}

fn battery(context: CommandContext) -> CrushResult<()> {
    let manager = battery::Manager::new()?;
    let output = context.output.initialize(BATTERY_OUTPUT_TYPE.clone())?;
    for battery in to_crush_error(manager.batteries())? {
        let battery = to_crush_error(battery)?;
        output.send(Row::new(vec![
        Value::String(battery.vendor().unwrap_or("").to_string()),
        Value::String(battery.model().unwrap_or("").to_string()),
        Value::Integer(battery.cycle_count().unwrap_or(0) as i128),
        Value::Integer((100.0 * battery.energy_full().value / battery.energy_full_design().value) as i128),
        Value::String(state_name(battery.state())),
        Value::Integer((100.0 * battery.energy().value / battery.energy_full().value) as i128),
        Value::Duration(time_to_duration(battery.time_to_full())),
        Value::Duration(time_to_duration(battery.time_to_empty())),
        ]))?;
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_lazy_namespace(
        "battery",
        Box::new(move |battery| {
            Battery::declare(battery)?;
            Ok(())
        }),
    )?;
    Ok(())
}
