use crate::lang::command::OutputType::Known;
use crate::lang::errors::{mandate, CrushResult, argument_error, to_crush_error};
use crate::lang::execution_context::{ArgumentVector, CommandContext};
use crate::lang::scope::Scope;
use crate::lang::r#struct::Struct;
use crate::lang::value::{Value, ValueType};
use crate::util::file::home;
use users::{get_current_gid, get_current_groupname, get_current_uid, get_current_username};
use signature::signature;
use std::convert::TryFrom;
use std::ffi::{CStr, CString};
use crate::lang::argument::ArgumentHandler;
use std::path::PathBuf;

fn home_fun(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context.output.send(Value::File(home()?))
}

fn name(context: CommandContext) -> CrushResult<()> {
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

fn group(context: CommandContext) -> CrushResult<()> {
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

fn uid(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(get_current_uid() as i128))
}

fn gid(context: CommandContext) -> CrushResult<()> {
    context.arguments.check_len(0)?;
    context
        .output
        .send(Value::Integer(get_current_gid() as i128))
}

#[signature(
me,
can_block = false,
short = "current user",
)]
struct Me {
}

fn me(context: CommandContext) -> CrushResult<()> {
    unsafe {
        context.output.send(search(
            mandate(
                mandate(
                    get_current_username(),
                    "Could not determine current username",
                )?
                    .to_str(),
                "Invalid username",
            )?.to_string()
        )?)
    }
}

#[signature(
find,
can_block = false,
short = "find a user by name",
)]
struct FromName {
    #[description("the of the user to find.")]
    name: String,
}

fn find(context: CommandContext) -> CrushResult<()> {
    let cfg: FromName = FromName::parse(context.arguments, &context.printer)?;
    unsafe {
        context.output.send(search(cfg.name)?)
    }
}

unsafe fn parse(s: *const i8) -> CrushResult<String> {
    Ok(to_crush_error(CStr::from_ptr(s).to_str())?.to_string())
}

unsafe fn search(input_name: String) -> CrushResult<Value> {
    nix::libc::setpwent();
    loop {
        let passwd = nix::libc::getpwent();
        if passwd.is_null() {
            return argument_error(format!("Unknown user {}", input_name));
        }
        let name = parse((*passwd).pw_name)?;
        if name == input_name {
            let res = Value::Struct(
                    Struct::new(
                        vec![
                            ("name".to_string(), Value::String(input_name)),
                            ("home".to_string(), Value::File(PathBuf::from(parse((*passwd).pw_dir)?))),
                            ("shell".to_string(), Value::File(PathBuf::from(parse((*passwd).pw_shell)?))),
                            ("information".to_string(), Value::String(parse((*passwd).pw_gecos)?)),
                            ("uid".to_string(), Value::Integer((*passwd).pw_uid as i128)),
                            ("gid".to_string(), Value::Integer((*passwd).pw_gid as i128)),
                        ],
                        None,
                    )
                );
            nix::libc::endpwent();
            return Ok(res);
        }
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    root.create_namespace(
        "user",
        Box::new(move |user| {
            Me::declare(user)?;
            FromName::declare(user)?;

            Ok(())
        }),
    )?;
    Ok(())
}
