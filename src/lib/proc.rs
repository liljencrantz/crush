use crate::lang::command::OutputType::Known;
use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::scope::Scope;
use crate::{lang::value::Value, lang::value::ValueType};
use nix::sys::signal;
use nix::unistd::Pid;
use signature::signature;
use std::str::FromStr;

#[cfg(target_os = "macos")]
mod macos {
    use lazy_static::lazy_static;
    use crate::lang::command::OutputType::Known;
    use crate::lang::errors::{CrushResult};
    use crate::lang::execution_context::CommandContext;
    use crate::lang::data::table::ColumnType;
    use crate::util::user_map::create_user_map;
    use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
    use chrono::Duration;
    use signature::signature;

    lazy_static! {
    static ref LIST_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("rss", ValueType::Integer),
        ColumnType::new("vms", ValueType::Integer),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];
}

    lazy_static! {
    static ref THREADS_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("tid", ValueType::Integer),
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("priority", ValueType::Integer),
        ColumnType::new("user", ValueType::Duration),
        ColumnType::new("system", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];
}

    #[signature(
    list,
    can_block = true,
    short = "Return a table stream containing information on all running processes on the system",
    output = Known(ValueType::TableInputStream(LIST_OUTPUT_TYPE.clone())),
    long = "proc:list accepts no arguments.")]
    pub struct List {}

    use libproc::libproc::bsd_info::BSDInfo;
    use libproc::libproc::file_info::{pidfdinfo, ListFDs, ProcFDType};
    use libproc::libproc::net_info::{InSockInfo, SocketFDInfo, SocketInfoKind, TcpSockInfo};
    use libproc::libproc::pid_rusage::{pidrusage, RUsageInfoV2};
    use libproc::libproc::proc_pid::{listpidinfo, listpids, pidinfo, ListThreads, ProcType};
    use libproc::libproc::task_info::{TaskAllInfo, TaskInfo};
    use libproc::libproc::thread_info::ThreadInfo;
    use std::path::PathBuf;
    use std::time::Instant;
    use libc::mach_timebase_info;

    fn list(context: CommandContext) -> CrushResult<()> {
        let mut base_procs = Vec::new();
        let arg_max = 2048;//get_arg_max();

        let output = context.output.initialize(LIST_OUTPUT_TYPE.clone())?;
        let users = create_user_map()?;
        let mut info: mach_timebase_info = mach_timebase_info{numer: 0, denom: 0};
        unsafe {
            mach_timebase_info(std::ptr::addr_of_mut!(info));
        }

        if let Ok(procs) = listpids(ProcType::ProcAllPIDS) {
            for p in procs {
                if let Ok(task) = pidinfo::<TaskAllInfo>(p as i32, 0) {
                    base_procs.push(p);
                }
            }
        }

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
                    ).unwrap_or_else(|g| { "<Invalid>".to_string() });
                output.send(Row::new(vec![
                    Value::Integer(pid as i128),
                    Value::Integer(ppid),
                    users.get(&nix::unistd::Uid::from_raw(curr_task.pbsd.pbi_uid)).map(|s| Value::string(s)).unwrap_or_else(|| Value::string("?")),
                    Value::Integer(i128::from(curr_task.ptinfo.pti_resident_size)),
                    Value::Integer(i128::from(curr_task.ptinfo.pti_virtual_size)),
                    Value::Duration(Duration::nanoseconds(
                        i64::try_from(curr_task.ptinfo.pti_total_user + curr_task.ptinfo.pti_total_system)? *
                            i64::from(info.numer) /
                            i64::from(info.denom))),
                    Value::String(name)
                ]))?;
            }
        }
        Ok(())
    }

    #[signature(
    threads,
    can_block = true,
    short = "Return a table stream containing information on all running threads on the system",
    output = Known(ValueType::TableInputStream(THREADS_OUTPUT_TYPE.clone())),
    long = "proc:threads accepts no arguments.")]
    pub struct Threads {}

    fn threads(context: CommandContext) -> CrushResult<()> {
        let mut base_procs = Vec::new();

        let output = context.output.initialize(THREADS_OUTPUT_TYPE.clone())?;
        let mut info: mach_timebase_info = mach_timebase_info{numer: 0, denom: 0};
        unsafe {
            mach_timebase_info(std::ptr::addr_of_mut!(info));
        }

        if let Ok(procs) = listpids(ProcType::ProcAllPIDS) {
            for p in procs {
                if let Ok(task) = pidinfo::<TaskAllInfo>(p as i32, 0) {
                    base_procs.push(p);
                }
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
                                ).unwrap_or_else(|g| { "<Invalid>".to_string() });
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
                                Value::String(name)
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
    use lazy_static::lazy_static;
    use crate::lang::command::OutputType::Known;
    use crate::lang::errors::{error, to_crush_error, CrushResult};
    use crate::lang::execution_context::CommandContext;
    use crate::lang::data::table::ColumnType;
    use crate::util::user_map::create_user_map;
    use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
    use chrono::Duration;
    use nix::unistd::Uid;
    use psutil::process::os::unix::ProcessExt;
    use psutil::process::{Process, ProcessResult, Status};
    use signature::signature;
    use std::collections::HashMap;

    lazy_static! {
    static ref LIST_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("status", ValueType::String),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("rss", ValueType::Integer),
        ColumnType::new("vms", ValueType::Integer),
        ColumnType::new("name", ValueType::String),
    ];
    }

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
    list,
    can_block = true,
    short = "Return a table stream containing information on all running processes on the system",
    output = Known(ValueType::TableInputStream(LIST_OUTPUT_TYPE.clone())),
    long = "proc:list accepts no arguments.")]
    pub struct List {}

    fn list(context: CommandContext) -> CrushResult<()> {
        List::parse(context.arguments.clone(), &context.global_state.printer())?;
        let output = context.output.initialize(LIST_OUTPUT_TYPE.clone())?;
        let users = create_user_map()?;

        match psutil::process::processes() {
            Ok(procs) => {
                for proc in procs {
                    output.send(to_crush_error(handle_process(proc, &users))?)?;
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
            Value::string(state_name(proc.status()?)),
            users.get(&nix::unistd::Uid::from_raw(proc.uids()?.effective)).map(|s| Value::string(s)).unwrap_or_else(|| Value::string("?")),
            Value::Duration(Duration::microseconds(
                proc.cpu_times()?.busy().as_micros() as i64
            )),
            Value::Integer(proc.memory_info()?.rss() as i128),
            Value::Integer(proc.memory_info()?.vms() as i128),
            Value::string(
                &proc.cmdline_vec()?
                    .unwrap_or(vec![format!("[{}]", proc.name()?)])[0],
            ),
        ]))
    }
}

#[signature(
signal,
can_block = false,
short = "Send a signal to a set of processes",
output = Known(ValueType::Empty),
long = "The set of existing signals is platform dependent, but common signals
    include SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE,
    SIGKILL, SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD,
    SIGCONT and SIGWINCH.")]
struct Signal {
    #[unnamed("id of a process to signal")]
    #[description("the name of the signal to send.")]
    pid: Vec<i128>,
    #[default("SIGTERM")]
    #[description("the name of the signal to send.")]
    signal: String,
}

fn signal(mut context: CommandContext) -> CrushResult<()> {
    let sig: Signal = Signal::parse(context.remove_arguments(), &context.global_state.printer())?;
    for pid in sig.pid {
        to_crush_error(signal::kill(
            Pid::from_raw(pid as i32),
            to_crush_error(signal::Signal::from_str(&sig.signal))?,
        ))?;
    }
    context.output.send(Value::Empty())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "proc",
        "Process related commands",
        Box::new(move |env| {
            #[cfg(target_os = "linux")]
            linux::List::declare(env)?;
            #[cfg(target_os = "macos")]
            macos::List::declare(env)?;
            #[cfg(target_os = "macos")]
            macos::Threads::declare(env)?;
            Signal::declare(env)?;
            Ok(())
        }))?;
    Ok(())
}
