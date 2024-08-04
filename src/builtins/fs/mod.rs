use crate::lang::command::OutputType::Known;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;
use crate::lang::value::ValueType;
use crate::util::file::{cwd, home};
use signature::signature;
use crate::lang::signature::files::Files;
use std::path::PathBuf;
use std::convert::TryFrom;

mod usage;
mod files;
mod mounts;
pub mod fd;

#[signature(
    fs.cd,
    can_block = false,
    output = Known(ValueType::Empty),
    short = "Change to the specified working directory.",
)]
struct Cd {
    #[unnamed()]
    #[description("the new working directory.")]
    destination: Files,
}

fn cd(mut context: CommandContext) -> CrushResult<()> {
    let cfg: Cd = Cd::parse(context.remove_arguments(), &context.global_state.printer())?;

    let dir = match cfg.destination.had_entries() {
        true => PathBuf::try_from(cfg.destination),
        false => home(),
    }?;

    std::env::set_current_dir(dir)?;
    context.output.send(Value::Empty)
}

#[signature(
    fs.pwd,
    can_block = false,
    output = Known(ValueType::File),
    short = "Return the current working directory.",
)]
struct Pwd {}

fn pwd(context: CommandContext) -> CrushResult<()> {
    context.output.send(Value::from(cwd()?))
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "fs",
        "File system functionality",
        Box::new(move |fs| {
            files::FilesSignature::declare(fs)?;
            Cd::declare(fs)?;
            mounts::Mounts::declare(fs)?;
            Pwd::declare(fs)?;
            usage::Usage::declare(fs)?;
            fs.create_namespace(
                "fd",
                "Information on currently open files and sockets",
                Box::new(move |fd| {
                    fd::File::declare(fd)?;
                    #[cfg(target_os = "linux")]
                    fd::procfs::Network::declare(fd)?;
                    #[cfg(target_os = "linux")]
                    fd::procfs::Unix::declare(fd)?;
                    Ok(())
                }),
            )?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
