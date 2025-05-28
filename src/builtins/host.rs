use crate::lang::command::OutputType::Known;
use crate::lang::errors::{CrushResult, mandate};
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use nix::sys::signal;
use nix::unistd::Pid;
use std::str::FromStr;
use crate::lang::data::r#struct::Struct;
use std::ops::Deref;
use crate::lang::data::table::ColumnType;
use crate::util::user_map::create_user_map;
use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
use chrono::Duration;
use nix::libc::uid_t;
use signature::signature;
use crate::lang::data::table::ColumnFormat;
#[cfg(target_os = "macos")]
use mach2::mach_time::mach_timebase_info;

#[signature(
    host.name,
    can_block = false,
    output = Known(ValueType::String),
    short = "name of this host")]
struct Name {}

fn name(context: CommandContext) -> CrushResult<()> {
    context
        .output
        .send(Value::from(mandate(nix::unistd::gethostname()?.to_str(), "Invalid hostname")?))
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
    context.output.send(Value::Duration(Duration::seconds(sysinfo::System::uptime() as i64)))
}

#[signature(
    host.battery,
    can_block = true,
    output = Known(ValueType::table_input_stream(& BATTERY_OUTPUT_TYPE)),
    short = "List all batteries in the system and their status")]
struct Battery {}

fn state_name(state: battery::State) -> String {
    match state {
        battery::State::Unknown => "Unknown",
        battery::State::Charging => "Charging",
        battery::State::Discharging => "Discharging",
        battery::State::Empty => "Empty",
        battery::State::Full => "Full",
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
            Value::from(battery.cycle_count().unwrap_or(0)),
            Value::from(battery.temperature().map(|t| { t.value as f64 }).unwrap_or(0.0)),
            Value::from(battery.voltage().value as f64),
            Value::from(battery.state_of_health().value as f64),
            Value::from(state_name(battery.state())),
            Value::from(battery.state_of_charge().value as f64),
            Value::from(time_to_duration(battery.time_to_full())),
            Value::from(time_to_duration(battery.time_to_empty())),
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
    let sys = sysinfo::System::new_all();
    context.output.send(Value::Struct(Struct::new(
        vec![
            ("total", Value::from(sys.total_memory())),
            ("free", Value::from(sys.free_memory())),
            ("avail", Value::from(sys.available_memory())),
            ("swap_total", Value::from(sys.total_swap()),),
            ("swap_free", Value::from(sys.free_swap()),),
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
            .send(Value::from(std::env::consts::OS))
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
            .send(Value::from(mandate(sysinfo::System::os_version(), "Unknown OS version")?))
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
        let sys = sysinfo::System::new_all();
        context
            .output
            .send(Value::from(sys.cpus().len()))
    }

    #[signature(
        host.cpu.arch,
        can_block = false,
        output = Known(ValueType::String),
        short = "the name of the CPU architecture")]
    pub struct Arch {}

    fn arch(context: CommandContext) -> CrushResult<()> {
        context
            .output
            .send(Value::from(sysinfo::System::cpu_arch()))
    }

    #[signature(
        host.cpu.load,
        can_block = false,
        output = Known(ValueType::Struct),
        short = "current CPU load")]
    pub struct Load {}

    fn load(context: CommandContext) -> CrushResult<()> {
        let load = sysinfo::System::load_average();
        context.output.send(Value::Struct(Struct::new(
            vec![
                ("one", Value::Float(load.one)),
                ("five", Value::Float(load.five)),
                ("fifteen", Value::Float(load.fifteen)),
            ],
            None,
        )))
    }
}

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
    output = Known(ValueType::table_input_stream(& LIST_OUTPUT_TYPE)),
    long = "host:procs accepts no arguments.")]
pub struct Procs {}

use nix::unistd;
use sysinfo::{ThreadKind, System};

fn procs(context: CommandContext) -> CrushResult<()> {
    let mut sys = System::new_all();
    sys.refresh_all();
    let output = context.output.initialize(&LIST_OUTPUT_TYPE)?;
    let users = create_user_map()?;

    for (pid, proc) in sys.processes() {
        if let None = proc.thread_kind() {
            output.send(Row::new(vec![
                Value::from(pid.as_u32()),
                Value::from(proc.parent().map(|i| i.as_u32()).unwrap_or(1u32)),
                proc.user_id().and_then(|i| {
                    let ii = i.deref();
                    let iii = *ii as uid_t;
                    let iiii = unistd::Uid::from_raw(iii);
                    return users.get(&iiii);
                }).map(|s| Value::from(s)).unwrap_or_else(|| Value::from("?")),
                Value::from(proc.memory()),
                Value::from(proc.virtual_memory()),
                Value::from(Duration::milliseconds(proc.accumulated_cpu_time() as i64)),
                Value::from(proc.name().to_str().unwrap_or("<Invalid>")),
            ]))?;
        }
    }
    Ok(())
}
#[cfg(target_os = "macos")]
#[signature(
    host.threads,
    can_block = true,
    short = "Return a table stream containing information on all running threads on this host",
    output = Known(ValueType::table_input_stream(& THREADS_OUTPUT_TYPE)),
    long = "host:threads accepts no arguments.")]
pub struct Threads {}

    #[cfg(target_os = "macos")]
fn threads(context: CommandContext) -> CrushResult<()> {
    let mut base_procs = Vec::new();

    let output = context.output.initialize(&THREADS_OUTPUT_TYPE)?;
/*
    use sysinfo::{
        System,
    };
    let mut sys = System::new_all();

    // First we update all information of our `System` struct.
    sys.refresh_all();

    for (pid, proc) in sys.processes() {

        match proc.thread_kind().unwrap() {
            ThreadKind::Kernel => {}
            ThreadKind::Userland => {}
        }
        proc.parent()

    }
*/
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
                            Value::from(t),
                            Value::from(pid),
                            Value::from(thread.pth_priority),
                            Value::from(Duration::nanoseconds(
                                i64::try_from(thread.pth_user_time)? *
                                    i64::from(info.numer) /
                                    i64::from(info.denom))),
                            Value::from(Duration::nanoseconds(
                                i64::try_from(thread.pth_system_time)? *
                                    i64::from(info.numer) /
                                    i64::from(info.denom))),
                            Value::from(name),
                        ]))?;

                        curr_threads.push(thread);
                    }
                }
            }
        }
    }
    Ok(())
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
            Procs::declare(host)?;
            #[cfg(target_os = "macos")]
            Threads::declare(host)?;
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
                    cpu::Arch::declare(env)?;
                    cpu::Count::declare(env)?;
                    cpu::Load::declare(env)?;
                    Ok(())
                }),
            )?;
            Ok(())
        }),
    )?;
    Ok(())
}
