use lazy_static::lazy_static;
use crate::lang::command::OutputType::Known;
use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::scope::Scope;
use crate::lang::data::table::ColumnType;
use crate::{data::table::Row, lang::value::Value, lang::value::ValueType};
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
    static ref PS_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("pid", ValueType::Integer),
        ColumnType::new("ppid", ValueType::Integer),
        ColumnType::new("user", ValueType::String),
        ColumnType::new("rss", ValueType::Integer),
        ColumnType::new("vms", ValueType::Integer),
        ColumnType::new("cpu", ValueType::Duration),
        ColumnType::new("name", ValueType::String),
    ];
}

    pub struct ProcessInfo {
        pub pid: i32,
        pub ppid: i32,
        pub curr_task: TaskAllInfo,
        pub prev_task: TaskAllInfo,
        pub curr_path: Option<PathInfo>,
        pub curr_threads: Vec<ThreadInfo>,
        pub curr_udps: Vec<InSockInfo>,
        pub curr_tcps: Vec<TcpSockInfo>,
        pub curr_res: Option<RUsageInfoV2>,
        pub prev_res: Option<RUsageInfoV2>,
        pub interval: Duration,
    }


    pub struct PathInfo {
        pub name: String,
        pub exe: PathBuf,
        pub root: PathBuf,
        pub cmd: Vec<String>,
        pub env: Vec<String>,
    }

    #[signature(
    ps,
    can_block = true,
    short = "Return a table stream containing information on all running processes on the system",
    output = Known(ValueType::TableInputStream(PS_OUTPUT_TYPE.clone())),
    long = "ps accepts no arguments.")]
    pub struct Ps {}

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

    fn clone_task_all_info(src: &TaskAllInfo) -> TaskAllInfo {
        let pbsd = BSDInfo {
            pbi_flags: src.pbsd.pbi_flags,
            pbi_status: src.pbsd.pbi_status,
            pbi_xstatus: src.pbsd.pbi_xstatus,
            pbi_pid: src.pbsd.pbi_pid,
            pbi_ppid: src.pbsd.pbi_ppid,
            pbi_uid: src.pbsd.pbi_uid,
            pbi_gid: src.pbsd.pbi_gid,
            pbi_ruid: src.pbsd.pbi_ruid,
            pbi_rgid: src.pbsd.pbi_rgid,
            pbi_svuid: src.pbsd.pbi_svuid,
            pbi_svgid: src.pbsd.pbi_svgid,
            rfu_1: src.pbsd.rfu_1,
            pbi_comm: src.pbsd.pbi_comm,
            pbi_name: src.pbsd.pbi_name,
            pbi_nfiles: src.pbsd.pbi_nfiles,
            pbi_pgid: src.pbsd.pbi_pgid,
            pbi_pjobc: src.pbsd.pbi_pjobc,
            e_tdev: src.pbsd.e_tdev,
            e_tpgid: src.pbsd.e_tpgid,
            pbi_nice: src.pbsd.pbi_nice,
            pbi_start_tvsec: src.pbsd.pbi_start_tvsec,
            pbi_start_tvusec: src.pbsd.pbi_start_tvusec,
        };
        let ptinfo = TaskInfo {
            pti_virtual_size: src.ptinfo.pti_virtual_size,
            pti_resident_size: src.ptinfo.pti_resident_size,
            pti_total_user: src.ptinfo.pti_total_user,
            pti_total_system: src.ptinfo.pti_total_system,
            pti_threads_user: src.ptinfo.pti_threads_user,
            pti_threads_system: src.ptinfo.pti_threads_system,
            pti_policy: src.ptinfo.pti_policy,
            pti_faults: src.ptinfo.pti_faults,
            pti_pageins: src.ptinfo.pti_pageins,
            pti_cow_faults: src.ptinfo.pti_cow_faults,
            pti_messages_sent: src.ptinfo.pti_messages_sent,
            pti_messages_received: src.ptinfo.pti_messages_received,
            pti_syscalls_mach: src.ptinfo.pti_syscalls_mach,
            pti_syscalls_unix: src.ptinfo.pti_syscalls_unix,
            pti_csw: src.ptinfo.pti_csw,
            pti_threadnum: src.ptinfo.pti_threadnum,
            pti_numrunning: src.ptinfo.pti_numrunning,
            pti_priority: src.ptinfo.pti_priority,
        };
        TaskAllInfo { pbsd, ptinfo }
    }

    fn ps(context: CommandContext) -> CrushResult<()> {
        let mut base_procs = Vec::new();
        let arg_max = 2048;//get_arg_max();

        let output = context.output.initialize(PS_OUTPUT_TYPE.clone())?;
        let users = create_user_map()?;

        let mut info: mach_timebase_info = mach_timebase_info{numer: 0, denom: 0};
        unsafe {
            mach_timebase_info(std::ptr::addr_of_mut!(info));
        }

        if let Ok(procs) = listpids(ProcType::ProcAllPIDS) {
            for p in procs {
                if let Ok(task) = pidinfo::<TaskAllInfo>(p as i32, 0) {
                    let res = pidrusage::<RUsageInfoV2>(p as i32).ok();
                    let time = Instant::now();
                    base_procs.push((p as i32, task, res, time));
                }
            }
        }

        for (pid, prev_task, prev_res, prev_time) in base_procs {
            let curr_task = if let Ok(task) = pidinfo::<TaskAllInfo>(pid, 0) {
                task
            } else {
                clone_task_all_info(&prev_task)
            };

            let curr_path: Option<PathInfo> = None;//get_path_info(pid, arg_max);

            let threadids = listpidinfo::<ListThreads>(pid, curr_task.ptinfo.pti_threadnum as usize);
            let mut curr_threads = Vec::new();
            if let Ok(threadids) = threadids {
                for t in threadids {
                    if let Ok(thread) = pidinfo::<ThreadInfo>(pid, t) {
                        curr_threads.push(thread);
                    }
                }
            }

            let mut curr_tcps = Vec::new();
            let mut curr_udps = Vec::new();

            let fds = listpidinfo::<ListFDs>(pid, curr_task.pbsd.pbi_nfiles as usize);
            if let Ok(fds) = fds {
                for fd in fds {
                    match fd.proc_fdtype.into() {
                        ProcFDType::Socket => {
                            if let Ok(socket) = pidfdinfo::<SocketFDInfo>(pid, fd.proc_fd) {
                                match socket.psi.soi_kind.into() {
                                    SocketInfoKind::In => {
                                        if socket.psi.soi_protocol == libc::IPPROTO_UDP {
                                            let info = unsafe { socket.psi.soi_proto.pri_in };
                                            curr_udps.push(info);
                                        }
                                    }
                                    SocketInfoKind::Tcp => {
                                        let info = unsafe { socket.psi.soi_proto.pri_tcp };
                                        curr_tcps.push(info);
                                    }
                                    _ => (),
                                }
                            }
                        }
                        _ => (),
                    }
                }
            }

            let curr_res = pidrusage::<RUsageInfoV2>(pid).ok();

            let ppid = curr_task.pbsd.pbi_ppid as i32;
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
                Value::Integer(ppid as i128),
                users.get(&nix::unistd::Uid::from_raw(curr_task.pbsd.pbi_uid)).map(|s| Value::string(s)).unwrap_or_else(|| Value::string("?")),
                Value::Integer(i128::from(curr_task.ptinfo.pti_resident_size)),
                Value::Integer(i128::from(curr_task.ptinfo.pti_virtual_size)),
                Value::Duration(Duration::nanoseconds(
                    i64::try_from(curr_task.ptinfo.pti_total_user + curr_task.ptinfo.pti_total_system)? *
                        i64::try_from(info.numer)? /
                        i64::try_from(info.denom)?)),
                Value::String(name)
            ]));
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
    static ref PS_OUTPUT_TYPE: Vec<ColumnType> = vec![
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
    ps,
    can_block = true,
    short = "Return a table stream containing information on all running processes on the system",
    output = Known(ValueType::TableInputStream(PS_OUTPUT_TYPE.clone())),
    long = "ps accepts no arguments.")]
    pub struct Ps {}

    fn ps(context: CommandContext) -> CrushResult<()> {
        Ps::parse(context.arguments.clone(), &context.global_state.printer())?;
        let output = context.output.initialize(PS_OUTPUT_TYPE.clone())?;
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
kill,
can_block = false,
short = "Send a signal to a set of processes",
output = Known(ValueType::Empty),
long = "The set of existing signals is platform dependent, but common signals
    include SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE,
    SIGKILL, SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM, SIGCHLD,
    SIGCONT and SIGWINCH.")]
struct Kill {
    #[unnamed("id of a process to signal")]
    #[description("the name of the signal to send.")]
    pid: Vec<i128>,
    #[default("SIGTERM")]
    #[description("the name of the signal to send.")]
    signal: String,
}

fn kill(context: CommandContext) -> CrushResult<()> {
    let sig: Kill = Kill::parse(context.arguments, &context.global_state.printer())?;
    for pid in sig.pid {
        to_crush_error(signal::kill(
            Pid::from_raw(pid as i32),
            to_crush_error(signal::Signal::from_str(&sig.signal))?,
        ))?;
    }
    context.output.send(Value::Empty())
}

lazy_static! {
    static ref JOBS_OUTPUT_TYPE: Vec<ColumnType> = vec![
        ColumnType::new("id", ValueType::Integer),
        ColumnType::new("description", ValueType::String),
    ];
}


#[signature(
jobs,
can_block = false,
short = "List running jobs",
output = Known(ValueType::TableInputStream(JOBS_OUTPUT_TYPE.clone())),
long = "All currently running jobs")]
struct Jobs {}

fn jobs(context: CommandContext) -> CrushResult<()> {
    let output = context.output.initialize(JOBS_OUTPUT_TYPE.clone())?;
    for job in context.global_state.jobs() {
        output.send(Row::new(vec![
            Value::Integer(usize::from(job.id) as i128),
            Value::string(job.description),
        ]))?;
    }
    Ok(())
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "proc",
        "Process related commands",
        Box::new(move |env| {
            #[cfg(target_os = "linux")]
            macos::Ps::declare(env)?;
            #[cfg(target_os = "macos")]
            macos::Ps::declare(env)?;
            Kill::declare(env)?;
            Jobs::declare(env)?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
