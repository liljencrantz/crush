use crate::lang::command::OutputType::Known;
use crate::lang::data::r#struct::Struct;
use crate::lang::data::table::ColumnFormat;
use crate::lang::data::table::ColumnType;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::util::user_map::create_user_map;
use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
use chrono::Duration;
use nix::sys::signal;
use nix::unistd::Pid;
use signature::signature;
use std::str::FromStr;
use sysinfo::System;

#[signature(
    host.name,
    can_block = false,
    output = Known(ValueType::String),
    short = "name of this host")]
struct Name {}

fn name(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::from(
        nix::unistd::gethostname()?
            .to_str()
            .ok_or("Invalid hostname")?,
    ))
}

#[signature(
    host.uptime,
    can_block = false,
    output = Known(ValueType::Duration),
    short = "uptime of this host")]
struct Uptime {}

fn uptime(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::Duration(Duration::seconds(
        sysinfo::System::uptime() as i64,
    )))
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
    host.battery,
    can_block = true,
    output = Known(ValueType::table_input_stream(&BATTERY_OUTPUT_TYPE)),
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
    }
    .to_string()
}

fn time_to_duration(tm: Option<battery::units::Time>) -> Duration {
    tm.map(|t| Duration::seconds(t.value as i64))
        .unwrap_or(Duration::seconds(0))
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
            Value::from(battery.temperature().map(|t| t.value as f64).unwrap_or(0.0)),
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
    short = "memory usage of this host.",
    long = "The output struct contains the following fields:",
    long = "* total, total amount of memory available to the host",
    long = "* free, unused memory",
    long = "* avail, available memory",
    long = "* swap_total, total amount of swap available to the host",
    long = "* swap_free, unused swap",
)]
struct Memory {}

fn memory(context: CommandContext) -> CrushResult<()> {
    let sys = sysinfo::System::new_all();
    context.output.send(Value::Struct(Struct::new(
        vec![
            ("total", Value::from(sys.total_memory())),
            ("free", Value::from(sys.free_memory())),
            ("avail", Value::from(sys.available_memory())),
            ("swap_total", Value::from(sys.total_swap())),
            ("swap_free", Value::from(sys.free_swap())),
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
        context.output.send(Value::from(std::env::consts::OS))
    }

    #[signature(
        host.os.version,
        can_block = false,
        output = Known(ValueType::String),
        short = "version of the operating system kernel"
    )]
    pub struct Version {}

    fn version(context: CommandContext) -> CrushResult<()> {
        context.output.send(Value::from(
            sysinfo::System::os_version().ok_or("Unknown OS version")?,
        ))
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
        context.output.send(Value::from(sys.cpus().len()))
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

static PROCS_OUTPUT_TYPE: [ColumnType; 7] = [
    ColumnType::new("pid", ValueType::Integer),
    ColumnType::new("ppid", ValueType::Integer),
    ColumnType::new("user", ValueType::String),
    ColumnType::new_with_format("rss", ColumnFormat::ByteUnit, ValueType::Integer),
    ColumnType::new_with_format("vms", ColumnFormat::ByteUnit, ValueType::Integer),
    ColumnType::new("cpu", ValueType::Duration),
    ColumnType::new("name", ValueType::String),
];

#[signature(
    host.procs,
    can_block = true,
    short = "Return a table stream containing information on all running processes on this host",
    output = Known(ValueType::table_input_stream(& PROCS_OUTPUT_TYPE)),
    long = "host:procs accepts no arguments.")]
pub struct Procs {}

fn procs(context: CommandContext) -> CrushResult<()> {
    let mut sys = System::new_all();
    sys.refresh_all();
    let output = context.output.initialize(&PROCS_OUTPUT_TYPE)?;
    let users = create_user_map()?;

    for (pid, proc) in sys.processes() {
        if let None = proc.thread_kind() {
            output.send(Row::new(vec![
                Value::from(pid.as_u32()),
                Value::from(proc.parent().map(|i| i.as_u32()).unwrap_or(1u32)),
                proc.user_id()
                    .and_then(|i| users.get(i))
                    .map(|s| Value::from(s))
                    .unwrap_or_else(|| Value::from("?")),
                Value::from(proc.memory()),
                Value::from(proc.virtual_memory()),
                Value::from(Duration::milliseconds(proc.accumulated_cpu_time() as i64)),
                Value::from(
                    proc.exe()
                        .map(|s| s.to_str())
                        .unwrap_or(proc.name().to_str())
                        .unwrap_or("<Invalid>"),
                ),
            ]))?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
mod macos {
    use super::*;
    use libproc::proc_pid::{ListThreads, listpidinfo, pidinfo};
    use libproc::processes::{ProcFilter, pids_by_type};
    use libproc::task_info::TaskAllInfo;
    use libproc::thread_info::ThreadInfo;
    use mach2::mach_time::mach_timebase_info;

    static THREADS_OUTPUT_TYPE: [ColumnType; 6] = [
        ColumnType::new("tid", ValueType::Integer),
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("priority", ValueType::Integer),
        ColumnType::new("user", ValueType::Duration),
        ColumnType::new("system", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];

    #[signature(
        host.threads,
        can_block = true,
        short = "Return a table stream containing information on all running threads on this host",
        output = Known(ValueType::table_input_stream(& THREADS_OUTPUT_TYPE)),
        long = "host:threads accepts no arguments.")]
    pub struct Threads {}

    fn threads(context: CommandContext) -> CrushResult<()> {
        let mut base_procs = Vec::new();

        let output = context.output.initialize(&THREADS_OUTPUT_TYPE)?;

        let mut info: mach_timebase_info = mach_timebase_info { numer: 0, denom: 0 };
        unsafe {
            mach_timebase_info(std::ptr::addr_of_mut!(info));
        }

        if let Ok(procs) = pids_by_type(ProcFilter::All) {
            for p in procs {
                base_procs.push(p);
            }
        }

        for pid in base_procs {
            if let Ok(curr_task) = pidinfo::<TaskAllInfo>(pid as i32, 0) {
                let threadids =
                    listpidinfo::<ListThreads>(pid as i32, curr_task.ptinfo.pti_threadnum as usize);
                let mut curr_threads = Vec::new();
                if let Ok(threadids) = threadids {
                    for t in threadids {
                        if let Ok(thread) = pidinfo::<ThreadInfo>(pid as i32, t) {
                            let name = String::from_utf8(
                                thread
                                    .pth_name
                                    .iter()
                                    .map(|c| i8::cast_unsigned(*c))
                                    .filter(|c| *c > 0u8)
                                    .collect(),
                            )
                            .unwrap_or_else(|_| "<Invalid>".to_string());
                            output.send(Row::new(vec![
                                Value::from(t),
                                Value::from(pid),
                                Value::from(thread.pth_priority),
                                Value::from(Duration::nanoseconds(
                                    i64::try_from(thread.pth_user_time)? * i64::from(info.numer)
                                        / i64::from(info.denom),
                                )),
                                Value::from(Duration::nanoseconds(
                                    i64::try_from(thread.pth_system_time)? * i64::from(info.numer)
                                        / i64::from(info.denom),
                                )),
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
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;

    static THREADS_OUTPUT_TYPE: [ColumnType; 7] = [
        ColumnType::new("tid", ValueType::Integer),
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("user", ValueType::String),
        ColumnType::new_with_format("rss", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new_with_format("vms", ColumnFormat::ByteUnit, ValueType::Integer),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];

    #[signature(
        host.threads,
        can_block = true,
        short = "Return a table stream containing information on all running threads on this host",
        output = Known(ValueType::table_input_stream(& THREADS_OUTPUT_TYPE)),
        long = "host:threads accepts no arguments.")]
    pub struct Threads {}

    fn threads(mut context: CommandContext) -> CrushResult<()> {
        let mut sys = System::new_all();
        sys.refresh_all();
        let output = context.output.initialize(&THREADS_OUTPUT_TYPE)?;
        let users = create_user_map()?;

        for (pid, proc) in sys.processes() {
            if let Some(kind) = proc.thread_kind() {
                output.send(Row::new(vec![
                    Value::from(pid.as_u32()),
                    Value::from(proc.parent().map(|i| i.as_u32()).unwrap_or(1u32)),
                    proc.user_id()
                        .and_then(|i| {
                            let ii = i.deref();
                            let iii = *ii as uid_t;
                            let iiii = unistd::Uid::from_raw(iii);
                            return users.get(&iiii);
                        })
                        .map(|s| Value::from(s))
                        .unwrap_or_else(|| Value::from("?")),
                    Value::from(proc.memory()),
                    Value::from(proc.virtual_memory()),
                    Value::from(Duration::milliseconds(proc.accumulated_cpu_time() as i64)),
                    Value::from(proc.name().to_str().unwrap_or("<Invalid>")),
                ]))?;
            }
        }
        Ok(())
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
    SIGCONT and SIGWINCH.",
    example = "# Create a `killall` command that kills any process whose name matches the specified pattern",
    example = "# The pattern can be a an exact string, a wildcard, or a regex",
    example = "$killall := { |$victim| host:signal @$(host:procs|where {$name =~ $victim}|select pid | list:collect) signal=SIGKILL}",
    example = "# Kill all crush commands",
    example = "killall ^(crush)",
)]
struct Signal {
    #[unnamed("id of a process to signal")]
    #[description("the id of the process to send to.")]
    pid: Vec<i128>,
    #[default("SIGTERM")]
    #[description("the name of the signal to send.")]
    signal: String,
}

fn signal(mut context: CommandContext) -> CrushResult<()> {
    let sig = Signal::parse(context.remove_arguments(), &context.global_state.printer())?;
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
        "Information about the host this crush session is running on",
        Box::new(move |host| {
            Battery::declare(host)?;
            Memory::declare(host)?;
            Name::declare(host)?;
            Uptime::declare(host)?;
            Procs::declare(host)?;
            #[cfg(target_os = "macos")]
            macos::Threads::declare(host)?;
            #[cfg(target_os = "linux")]
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
