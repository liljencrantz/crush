use crate::lang::command::OutputType::Known;
use crate::lang::errors::{mandate, CrushResult};
use crate::lang::execution_context::{ArgumentVector, ExecutionContext};
use crate::lang::scope::Scope;
use crate::lang::value::{Value, ValueType};
use crate::util::file::home;
use users::{get_current_gid, get_current_groupname, get_current_uid, get_current_username};

fn home_fun(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::File(home()?))
}

fn name(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::string(mandate(
        mandate(
            get_current_username(),
            "Could not determine current username",
        )?
        .to_str(),
        "Invalid username",
    )?))
}

fn group(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::string(mandate(
        mandate(
            get_current_groupname(),
            "Could not determine current group name",
        )?
        .to_str(),
        "Invalid group name",
    )?))
}

fn uid(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(get_current_uid() as i128))
}

fn gid(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(get_current_gid() as i128))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_lazy_namespace(
        "user",
        Box::new(move |env| {
            env.declare_command(
                "home",
                home_fun,
                false,
                "home",
                "Current users home directory",
                None,
                Known(ValueType::File),
            )?;
            env.declare_command(
                "name",
                name,
                false,
                "name",
                "Current users name",
                None,
                Known(ValueType::String),
            )?;
            env.declare_command(
                "group",
                group,
                false,
                "group",
                "Current group name",
                None,
                Known(ValueType::String),
            )?;
            env.declare_command(
                "uid",
                uid,
                false,
                "uid",
                "Current users user id",
                None,
                Known(ValueType::Integer),
            )?;
            env.declare_command(
                "gid",
                gid,
                false,
                "gid",
                "Current users group id",
                None,
                Known(ValueType::Integer),
            )?;
            Ok(())
        }),
    )?;
    Ok(())
}
