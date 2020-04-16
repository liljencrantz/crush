use crate::util::file::home;
use users::{get_current_username, get_current_groupname, get_current_uid, get_current_gid};
use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, mandate};
use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::value::Value;

fn home_fun(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::File(home()?))
}

fn name(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::string(
        mandate(
            mandate(
                get_current_username(),
                "Could not determine current username")?.to_str(),
            "Invalid username")?))
}

fn group(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::string(
        mandate(
            mandate(
                get_current_groupname(),
                "Could not determine current group name")?.to_str(),
            "Invalid group name")?))
}

fn uid(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(get_current_uid() as i128))
}

fn gid(context: ExecutionContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::Integer(get_current_gid() as i128))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("user")?;
    env.declare_command("home", home_fun, false, "home", "Current users home directory", None)?;
    env.declare_command("name", name, false, "name", "Current users name", None)?;
    env.declare_command("group", group, false, "group", "Current group name", None)?;
    env.declare_command("uid", uid, false, "uid", "Current users user id", None)?;
    env.declare_command("gid", gid, false, "gid", "Current users group id", None)?;
    env.readonly();
    Ok(())
}
