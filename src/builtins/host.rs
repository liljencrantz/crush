use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::{lang::value::Value, lang::value::ValueType};
use nix::sys::signal;
use nix::unistd::Pid;
use signature::signature;
use std::str::FromStr;
use crate::lang::errors::error;
use crate::lang::data::r#struct::Struct;
use sys_info;
use battery::State;
use chrono::Duration;
use crate::lang::data::table::ColumnFormat;
use crate::lang::data::table::ColumnType;
use crate::lang::data::table::Row;

extern crate uptime_lib;

#[signature(
    host.name,
    can_block = false,
    output = Known(ValueType::String),
    short = "name of this host")]
struct Name {}

fn name(context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::from(sys_info::hostname()?))
}

static BATTERY_OUTPUT_TYPE: [ColumnType; 11] = [
    ColumnType::new("vendor", ValueType::String),
    ColumnType::new("model", ValueType::String),
    ColumnType::new("technology", ValueType::String),
    ColumnType::new("cycle_count", ValueType::Integer),
    ColumnType::new_with_format("temperature", ColumnFormat::Temperature, ValueType::Float),
    ColumnType::new("voltage", ValueType::Float),
    ColumnType::new_with_format("health", ColumnFormat::Percentage, ValueType::Float),
    ColumnType::new("state", ValueType::String),
    ColumnType::new_with_format("charge", ColumnFormat::Percentage, ValueType::Float),
    ColumnType::new("time_to_full", ValueType::Duration),
    ColumnType::new("time_to_empty", ValueType::Duration),
];

#[signature(
    host.uptime,
    can_block = false,
    output = Known(ValueType::Duration),
    short = "uptime of this host")]
struct Uptime {}

fn uptime(context: CommandContext) -> CrushResult<()> {
    match uptime_lib::get() {
        Ok(d) => context.output.send(Value::Duration(Duration::nanoseconds(i64::try_from(d.as_nanos()).unwrap()))),
        Err(e) => error(e.to_string()),
    }
}


#[signature(
    host.battery,
    can_block = true,
    output = Known(ValueType::table_input_stream(& BATTERY_OUTPUT_TYPE)),
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
    let output = context.output.initialize(&BATTERY_OUTPUT_TYPE)?;
    for battery in manager.batteries()? {
        let battery = battery?;
        output.send(Row::new(vec![
            Value::from(battery.vendor().unwrap_or("").to_string()),
            Value::from(battery.model().unwrap_or("").to_string()),
            Value::from(battery.technology().to_string()),
            Value::Integer(battery.cycle_count().unwrap_or(0) as i128),
            Value::Float(battery.temperature().map(|t| { t.value as f64 }).unwrap_or(0.0)),
            Value::Float(battery.voltage().value as f64),
            Value::Float(battery.state_of_health().value as f64),
            Value::from(state_name(battery.state())),
            Value::Float(battery.state_of_charge().value as f64),
            Value::Duration(time_to_duration(battery.time_to_full())),
            Value::Duration(time_to_duration(battery.time_to_empty())),
        ]))?;
    }
    Ok(())
}

#[signature(
    host.memory,
    can_block = false,
    output = Known(ValueType::Struct),
    short = "memory usage of this host.")]
struct Memory {}

fn memory(context: CommandContext) -> CrushResult<()> {
    let mem = sys_info::mem_info()?;
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
        host.os.name,
        can_block = false,
        output = Known(ValueType::String),
        short = "name of the operating system")]
    pub struct Name {}

    fn name(context: CommandContext) -> CrushResult<()> {
        context
            .output
            .send(Value::from(sys_info::os_type()?))
    }

    #[signature(
        host.os.version,
        can_block = false,
        output = Known(ValueType::String),
        short = "version of the operating system kernel"
    )]
    pub struct Version {}

    fn version(context: CommandContext) -> CrushResult<()> {
        context
            .output
            .send(Value::from(sys_info::os_release()?))
    }
}

mod cpu {
    use super::*;

    #[signature(
        host.cpu.count,
        can_block = false,
        output = Known(ValueType::Integer),
        short = "number of CPU cores")]
    pub struct Count {}

    fn count(context: CommandContext) -> CrushResult<()> {
        context
            .output
            .send(Value::Integer(sys_info::cpu_num()? as i128))
    }

    #[signature(
        host.cpu.load,
        can_block = false,
        output = Known(ValueType::Struct),
        short = "current CPU load")]
    pub struct Load {}

    fn load(context: CommandContext) -> CrushResult<()> {
        let load = sys_info::loadavg()?;
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
        host.cpu.speed,
        can_block = false,
        output = Known(ValueType::Integer),
        short = "current CPU frequency")]
    pub struct Speed {}

    fn speed(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::Integer(
            sys_info::cpu_speed()? as i128
        ))
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use crate::lang::command::OutputType::Known;
    use crate::lang::errors::{CrushResult};
    use crate::lang::state::contexts::CommandContext;
    use crate::lang::data::table::ColumnType;
    use crate::util::user_map::create_user_map;
    use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
    use chrono::Duration;
    use signature::signature;
    use crate::lang::data::table::{ColumnFormat};

    static LIST_OUTPUT_TYPE: [ColumnType; 7] = [
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("user", ValueType::String),
        ColumnType::new_with_format("rss", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new_with_format("vms", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];

    static THREADS_OUTPUT_TYPE: [ColumnType; 6] = [
        ColumnType::new("tid", ValueType::Integer),
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("priority", ValueType::Integer),
        ColumnType::new("user", ValueType::Duration),
        ColumnType::new("system", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];

    #[signature(
        host.procs,
        can_block = true,
        short = "Return a table stream containing information on all running processes on this host",
        output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
        long = "host:procs accepts no arguments.")]
    pub struct Procs {}

    use libproc::libproc::proc_pid::{listpidinfo, listpids, pidinfo, ListThreads, ProcType};
    use libproc::libproc::task_info::TaskAllInfo;
    use libproc::libproc::thread_info::ThreadInfo;
    use mach2::mach_time::mach_timebase_info;

    fn procs(context: CommandContext) -> CrushResult<()> {
        let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
        let users = create_user_map()?;
        let mut info: mach_timebase_info = mach_timebase_info { numer: 0, denom: 0 };
        unsafe {
            mach_timebase_info(std::ptr::addr_of_mut!(info));
        }

        if let Ok(base_procs) = listpids(ProcType::ProcAllPIDS) {
            for pid in base_procs {
                if let Ok(curr_task) = pidinfo::<TaskAllInfo>(pid as i32, 0) {
                    let ppid = curr_task.pbsd.pbi_ppid as i128;
                    let name =
                        String::from_utf8(
                            curr_task.pbsd.pbi_name
                                .iter()
                                .map(|c| unsafe { std::mem::transmute::<i8, u8>(*c) })
                                .filter(|c| { *c > 0u8 })
                                .collect()
                        ).unwrap_or_else(|_| { "<Invalid>".to_string() });
                    output.send(Row::new(vec![
                        Value::Integer(pid as i128),
                        Value::Integer(ppid),
                        users.get(&nix::unistd::Uid::from_raw(curr_task.pbsd.pbi_uid)).map(|s| Value::from(s)).unwrap_or_else(|| Value::from("?")),
                        Value::Integer(i128::from(curr_task.ptinfo.pti_resident_size)),
                        Value::Integer(i128::from(curr_task.ptinfo.pti_virtual_size)),
                        Value::Duration(Duration::nanoseconds(
                            i64::try_from(curr_task.ptinfo.pti_total_user + curr_task.ptinfo.pti_total_system)? *
                                i64::from(info.numer) /
                                i64::from(info.denom))),
                        Value::from(name),
                    ]))?;
                }
            }
        }
        Ok(())
    }

    #[signature(
        host.threads,
        can_block = true,
        short = "Return a table stream containing information on all running threads on this host",
        output = Known(ValueType::table_input_stream(&THREADS_OUTPUT_TYPE)),
        long = "host:threads accepts no arguments.")]
    pub struct Threads {}

    fn threads(context: CommandContext) -> CrushResult<()> {
        let mut base_procs = Vec::new();

        let output = context.output.initialize(&THREADS_OUTPUT_TYPE)?;
        let mut info: mach_timebase_info = mach_timebase_info { numer: 0, denom: 0 };
        unsafe {
            mach_timebase_info(std::ptr::addr_of_mut!(info));
        }

        if let Ok(procs) = listpids(ProcType::ProcAllPIDS) {
            for p in procs {
                base_procs.push(p);
            }
        }

        for pid in base_procs {
            if let Ok(curr_task) = pidinfo::<TaskAllInfo>(pid as i32, 0) {
                let threadids = listpidinfo::<ListThreads>(pid as i32, curr_task.ptinfo.pti_threadnum as usize);
                let mut curr_threads = Vec::new();
                if let Ok(threadids) = threadids {
                    for t in threadids {
                        if let Ok(thread) = pidinfo::<ThreadInfo>(pid as i32, t) {
                            let name =
                                String::from_utf8(
                                    thread.pth_name
                                        .iter()
                                        .map(|c| unsafe { std::mem::transmute::<i8, u8>(*c) })
                                        .filter(|c| { *c > 0u8 })
                                        .collect()
                                ).unwrap_or_else(|_| { "<Invalid>".to_string() });
                            output.send(Row::new(vec![
                                Value::Integer(t as i128),
                                Value::Integer(pid as i128),
                                Value::Integer(thread.pth_priority as i128),
                                Value::Duration(Duration::nanoseconds(
                                    i64::try_from(thread.pth_user_time)? *
                                        i64::from(info.numer) /
                                        i64::from(info.denom))),
                                Value::Duration(Duration::nanoseconds(
                                    i64::try_from(thread.pth_system_time)? *
                                        i64::from(info.numer) /
                                        i64::from(info.denom))),
                                Value::from(name),
                            ]))?;

                            curr_threads.push(thread);
                        }
                    }
                }

                //                let curr_res = pidrusage::<RUsageInfoV2>(pid).ok();
            }
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use crate::lang::command::OutputType::Known;
    use crate::lang::errors::{error, CrushResult};
    use crate::lang::state::contexts::CommandContext;
    use crate::lang::data::table::ColumnType;
    use crate::util::user_map::create_user_map;
    use crate::{lang::value::Value, lang::value::ValueType};
    use chrono::Duration;
    use nix::unistd::Uid;
    use psutil::process::os::unix::ProcessExt;
    use psutil::process::{Process, ProcessResult, Status};
    use signature::signature;
    use std::collections::HashMap;
    use crate::lang::data::table::{Row, ColumnFormat};

    static LIST_OUTPUT_TYPE: [ColumnType; 8] = [
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("status", ValueType::String),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new_with_format("rss", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new_with_format("vms", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new("name", ValueType::String),
    ];

    fn state_name(s: Status) -> &'static str {
        match s {
            Status::Running => "Running",
            Status::Sleeping => "Sleeping",
            Status::Waiting => "Waiting",
            Status::Stopped => "Stopped",
            Status::Dead => "Dead",
            Status::Zombie => "Zombie",
            Status::Idle => "Idle",
            Status::DiskSleep => "DiskSleep",
            Status::TracingStop => "TracingStop",
            Status::WakeKill => "WakeKill",
            Status::Waking => "Waking",
            Status::Parked => "Parked",
            Status::Locked => "Locked",
            Status::Suspended => "Suspended",
        }
    }


    #[signature(
        host.procs,
        can_block = true,
        short = "Return a table stream containing information on all running processes on this host",
        output = Known(ValueType::table_input_stream(&LIST_OUTPUT_TYPE)),
        long = "host:procs accepts no arguments.")]
    pub struct Procs {}

    fn procs(mut context: CommandContext) -> CrushResult<()> {
        Procs::parse(context.remove_arguments(), &context.global_state.printer())?;
        let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
        let users = create_user_map()?;

        match psutil::process::processes() {
            Ok(procs) => {
                for proc in procs {
                    output.send(handle_process(proc, &users)?)?;
                }
            }
            Err(_) => return error("Failed to list processes"),
        }
        Ok(())
    }

    fn handle_process(proc: ProcessResult<Process>, users: &HashMap<Uid, String>) -> ProcessResult<Row> {
        let proc = proc?;

        Ok(Row::new(vec![
            Value::Integer(proc.pid() as i128),
            Value::Integer(proc.ppid()?.unwrap_or(0) as i128),
            Value::from(state_name(proc.status()?)),
            users.get(&nix::unistd::Uid::from_raw(proc.uids()?.effective)).map(|s| Value::from(s)).unwrap_or_else(|| Value::from("?")),
            Value::Duration(Duration::microseconds(
                proc.cpu_times()?.busy().as_micros() as i64
            )),
            Value::Integer(proc.memory_info()?.rss() as i128),
            Value::Integer(proc.memory_info()?.vms() as i128),
            Value::from(
                &proc.cmdline_vec()?
                    .unwrap_or(vec![format!("[{}]", proc.name()?)])[0],
            ),
        ]))
    }
}

#[signature(
    host.signal,
    can_block = false,
    short = "Send a signal to a set of processes",
    output = Known(ValueType::Empty),
    long = "The set of existing signals is platform dependent, but common signals
    include SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE,
    SIGKILL, SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD,
    SIGCONT and SIGWINCH.")]
struct Signal {
    #[unnamed("id of a process to signal")]
    #[description("the id of the process to send to.")]
    pid: Vec<i128>,
    #[default("SIGTERM")]
    #[description("the name of the signal to send.")]
    signal: String,
}

fn signal(mut context: CommandContext) -> CrushResult<()> {
    let sig: Signal = Signal::parse(context.remove_arguments(), &context.global_state.printer())?;
    for pid in sig.pid {
        signal::kill(
            Pid::from_raw(pid as i32),
            signal::Signal::from_str(&sig.signal)?,
        )?;
    }
    context.output.empty()
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "host",
        "Information about this host",
        Box::new(move |host| {
            Battery::declare(host)?;
            Memory::declare(host)?;
            Name::declare(host)?;
            Uptime::declare(host)?;
            #[cfg(target_os = "linux")]
            linux::Procs::declare(host)?;
            #[cfg(target_os = "macos")]
            macos::Procs::declare(host)?;
            #[cfg(target_os = "macos")]
            macos::Threads::declare(host)?;
            Signal::declare(host)?;
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
