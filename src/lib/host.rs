use crate::lang::errors::{CrushResult, to_crush_error};
use crate::lang::scope::Scope;
use crate::lang::execution_context::ExecutionContext;
use sys_info;
use crate::lang::value::Value;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

#[signature(
name,
can_block = false,
short = "name of this host")]
struct Name {
}

fn name(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::String(to_crush_error(sys_info::hostname())?))
}

mod os {
    use crate::lang::execution_context::ExecutionContext;
    use crate::lang::errors::{CrushResult, to_crush_error};
    use crate::lang::value::Value;
    use signature::signature;

    #[signature(
    name,
    can_block = false,
    short = "name of the operating system")]
    pub struct Name {}

    fn name(context: ExecutionContext) -> CrushResult<()> {
        context.output.send(Value::String(to_crush_error(sys_info::os_type())?))
    }

    #[signature(
    version,
    can_block = false,
    short = "version of the operating system kernel")]
    pub struct Version {}

    fn version(context: ExecutionContext) -> CrushResult<()> {
        context.output.send(Value::String(to_crush_error(sys_info::os_release())?))
    }
}

mod cpu {
    use crate::lang::execution_context::ExecutionContext;
    use crate::lang::errors::{CrushResult, to_crush_error};
    use crate::lang::value::{Value, ValueType};
    use signature::signature;
    use crate::lang::list::List;

    #[signature(
    count,
    can_block = false,
    short = "number of CPU cores")]
    pub struct Count {}

    fn count(context: ExecutionContext) -> CrushResult<()> {
        context.output.send(Value::Integer(to_crush_error(sys_info::cpu_num())? as i128))
    }

    #[signature(
    load,
    can_block = false,
    short = "number of CPU cores")]
    pub struct Load {}

    fn load(context: ExecutionContext) -> CrushResult<()> {
        let load = to_crush_error(sys_info::loadavg())?;
        context.output.send(Value::List(
            List::new(ValueType::Float, vec![
                Value::Float(load.one),
                Value::Float(load.five),
                Value::Float(load.fifteen)])
        ))
    }

    #[signature(
    speed,
    can_block = false,
    short = "number of CPU cores")]
    pub struct Speed {}

    fn speed(context: ExecutionContext) -> CrushResult<()> {
        context.output.send(Value::Integer(to_crush_error(sys_info::cpu_speed())? as i128))
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_lazy_namespace(
        "host",
        Box::new(move |env| {
            Name::declare(env)?;
            env.create_lazy_namespace(
                "os",
                Box::new(move |env| {
                    os::Name::declare(env)?;
                    os::Version::declare(env)?;
                    Ok(())
                })
            )?;
            env.create_lazy_namespace(
                "cpu",
                Box::new(move |env| {
                    cpu::Count::declare(env)?;
                    cpu::Speed::declare(env)?;
                    cpu::Load::declare(env)?;
                    Ok(())
                })
            )?;
            Ok(())
        }))?;
    Ok(())
}
