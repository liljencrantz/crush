use crate::lang::errors::{to_crush_error, CrushResult, error};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::r#struct::Struct;
use crate::lang::data::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::lang::data::table::{ColumnType, Row};
use signature::signature;
use sys_info;
use lazy_static::lazy_static;
use battery::State;
use chrono::Duration;
use crate::lang::command::OutputType::Known;

extern crate uptime_lib;

#[signature(
name,
can_block = false,
output = Known(ValueType::String),
short = "name of this host")]
struct Name {}

fn name(context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::String(to_crush_error(sys_info::hostname())?))
}

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
uptime,
can_block = false,
output = Known(ValueType::Duration),
short = "uptime of this host")]
struct Uptime {}

fn uptime(context: CommandContext) -> CrushResult<()> {
    match uptime_lib::get() {
        Ok(d) => context.output.send(Value::Duration(Duration::nanoseconds(i64::try_from(d.as_nanos()).unwrap()))),
        Err(e) => error(e),
    }
}


#[signature(
battery,
can_block = true,
output = Known(ValueType::TableInputStream(BATTERY_OUTPUT_TYPE.clone())),
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

#[signature(
memory,
can_block = false,
output = Known(ValueType::Struct),
short = "memory usage of this host.")]
struct Memory {}

fn memory(context: CommandContext) -> CrushResult<()> {
    let mem = to_crush_error(sys_info::mem_info())?;
    context.output.send(Value::Struct(Struct::new(
        vec![
            ("total", Value::Integer(mem.total as i128)),
            ("free", Value::Integer(mem.free as i128)),
            ("avail", Value::Integer(mem.avail as i128)),
            ("buffers", Value::Integer(mem.buffers as i128)),
            ("cached", Value::Integer(mem.cached as i128)),
            (
                "swap_total",
                Value::Integer(mem.swap_total as i128),
            ),
            (
                "swap_free",
                Value::Integer(mem.swap_free as i128),
            ),
        ],
        None,
    )))
}

mod os {
    use super::*;

    #[signature(
    name,
    can_block = false,
    output = Known(ValueType::String),
    short = "name of the operating system")]
    pub struct Name {}

    fn name(context: CommandContext) -> CrushResult<()> {
        context
            .output
            .send(Value::String(to_crush_error(sys_info::os_type())?))
    }

    #[signature(
    version,
    can_block = false,
    output = Known(ValueType::String),
    short = "version of the operating system kernel"
    )]
    pub struct Version {}

    fn version(context: CommandContext) -> CrushResult<()> {
        context
            .output
            .send(Value::String(to_crush_error(sys_info::os_release())?))
    }
}

mod cpu {
    use super::*;

    #[signature(
    count,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "number of CPU cores")]
    pub struct Count {}

    fn count(context: CommandContext) -> CrushResult<()> {
        context
            .output
            .send(Value::Integer(to_crush_error(sys_info::cpu_num())? as i128))
    }

    #[signature(
    load,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "current CPU load")]
    pub struct Load {}

    fn load(context: CommandContext) -> CrushResult<()> {
        let load = to_crush_error(sys_info::loadavg())?;
        context.output.send(Value::Struct(Struct::new(
            vec![
                ("one", Value::Float(load.one)),
                ("five", Value::Float(load.five)),
                ("fifteen", Value::Float(load.fifteen)),
            ],
            None,
        )))
    }

    #[signature(
    speed,
    can_block = false,
    output = Known(ValueType::Integer),
    short = "current CPU frequency")]
    pub struct Speed {}

    fn speed(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::Integer(
            to_crush_error(sys_info::cpu_speed())? as i128
        ))
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "host",
        "Metadata about this host",
        Box::new(move |host| {
            Battery::declare(host)?;
            Memory::declare(host)?;
            Name::declare(host)?;
            Uptime::declare(host)?;
            host.create_namespace(
                "os",
                "Metadata about the operating system this host is running",
                Box::new(move |env| {
                    os::Name::declare(env)?;
                    os::Version::declare(env)?;
                    Ok(())
                }),
            )?;
            host.create_namespace(
                "cpu",
                "Metadata about the CPUs of this host",
                Box::new(move |env| {
                    cpu::Count::declare(env)?;
                    cpu::Speed::declare(env)?;
                    cpu::Load::declare(env)?;
                    Ok(())
                }),
            )?;
            Ok(())
        }),
    )?;
    Ok(())
}
