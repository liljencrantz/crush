use crate::lang::argument::ArgumentHandler;
use crate::lang::errors::{to_crush_error, CrushResult};
use crate::lang::execution_context::ExecutionContext;
use crate::lang::r#struct::Struct;
use crate::lang::scope::Scope;
use crate::lang::value::Value;
use signature::signature;
use sys_info;

#[signature(name, can_block = false, short = "name of this host")]
struct Name {}

fn name(context: ExecutionContext) -> CrushResult<()> {
    context
        .output
        .send(Value::String(to_crush_error(sys_info::hostname())?))
}

#[signature(mem, can_block = false, short = "name of this host")]
struct Mem {}

fn mem(context: ExecutionContext) -> CrushResult<()> {
    let mem = to_crush_error(sys_info::mem_info())?;
    context.output.send(Value::Struct(Struct::new(
        vec![
            ("total".to_string(), Value::Integer(mem.total as i128)),
            ("free".to_string(), Value::Integer(mem.free as i128)),
            ("avail".to_string(), Value::Integer(mem.avail as i128)),
            ("buffers".to_string(), Value::Integer(mem.buffers as i128)),
            ("cached".to_string(), Value::Integer(mem.cached as i128)),
            (
                "swap_total".to_string(),
                Value::Integer(mem.swap_total as i128),
            ),
            (
                "swap_free".to_string(),
                Value::Integer(mem.swap_free as i128),
            ),
        ],
        None,
    )))
}

mod os {
    use super::*;

    #[signature(name, can_block = false, short = "name of the operating system")]
    pub struct Name {}

    fn name(context: ExecutionContext) -> CrushResult<()> {
        context
            .output
            .send(Value::String(to_crush_error(sys_info::os_type())?))
    }

    #[signature(
        version,
        can_block = false,
        short = "version of the operating system kernel"
    )]
    pub struct Version {}

    fn version(context: ExecutionContext) -> CrushResult<()> {
        context
            .output
            .send(Value::String(to_crush_error(sys_info::os_release())?))
    }
}

mod cpu {
    use super::*;

    #[signature(count, can_block = false, short = "number of CPU cores")]
    pub struct Count {}

    fn count(context: ExecutionContext) -> CrushResult<()> {
        context
            .output
            .send(Value::Integer(to_crush_error(sys_info::cpu_num())? as i128))
    }

    #[signature(load, can_block = false, short = "current CPU load")]
    pub struct Load {}

    fn load(context: ExecutionContext) -> CrushResult<()> {
        let load = to_crush_error(sys_info::loadavg())?;
        context.output.send(Value::Struct(Struct::new(
            vec![
                ("one".to_string(), Value::Float(load.one)),
                ("five".to_string(), Value::Float(load.five)),
                ("fifteen".to_string(), Value::Float(load.fifteen)),
            ],
            None,
        )))
    }

    #[signature(speed, can_block = false, short = "current CPU frequency")]
    pub struct Speed {}

    fn speed(context: ExecutionContext) -> CrushResult<()> {
        context.output.send(Value::Integer(
            to_crush_error(sys_info::cpu_speed())? as i128
        ))
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_lazy_namespace(
        "host",
        Box::new(move |host| {
            Name::declare(host)?;
            host.create_lazy_namespace(
                "os",
                Box::new(move |env| {
                    os::Name::declare(env)?;
                    os::Version::declare(env)?;
                    Ok(())
                }),
            )?;
            host.create_lazy_namespace(
                "cpu",
                Box::new(move |env| {
                    cpu::Count::declare(env)?;
                    cpu::Speed::declare(env)?;
                    cpu::Load::declare(env)?;
                    Ok(())
                }),
            )?;
            Mem::declare(host)?;
            Ok(())
        }),
    )?;
    Ok(())
}
