use crate::lang::scope::Scope;
use crate::lang::errors::{CrushResult, error, to_crush_error, argument_error};
use crate::lang::{value::Value};
use crate::util::file::{home, cwd};
use std::path::Path;
use crate::lang::execution_context::ExecutionContext;
use crate::lang::execution_context::ArgumentVector;
use crate::lang::help::Help;
use crate::lang::printer::Printer;

mod find;

pub fn cd(context: ExecutionContext) -> CrushResult<()> {
    let dir = match context.arguments.len() {
        0 => home(),
        1 => {
            let dir = &context.arguments[0];
            match &dir.value {
                Value::String(val) => Ok(Box::from(Path::new(val.as_ref()))),
                Value::File(val) => Ok(val.clone()),
                Value::Glob(val) => val.glob_to_single_file(&cwd()?),
                _ => error(format!("Wrong parameter type, expected text or file, found {}", &dir.value.value_type().to_string()).as_str())
            }
        }
        _ => error("Wrong number of arguments")
    }?;
    to_crush_error(std::env::set_current_dir(dir))
}

pub fn pwd(context: ExecutionContext) -> CrushResult<()> {
    context.output.send(Value::File(cwd()?))
}

fn halp(o: &dyn Help, printer: &Printer) {
    printer.line(
        match o.long_help() {
            None => format!("{}\n\n    {}", o.signature(), o.short_help()),
            Some(long_help) => format!("{}\n\n    {}\n\n{}", o.signature(), o.short_help(), long_help),
        }.as_str());
}

pub fn help(mut context: ExecutionContext) -> CrushResult<()> {
    match context.arguments.len() {
        0 => {
            context.printer.line(r#"
Welcome to Crush!

If this is your first time using Crush, congratulations on just entering your
first command! If you haven't already, you might want to check out the Readme
for an introduction at https://github.com/liljencrantz/crush/.

Call the help command with the name of any value, including a command or a
type in order to get help about it. For example, you might want to run the
commands "help help", "help string", "help if" or "help where".

To get a list of everything in your namespace, write "var:env". To list the
members of a value, write "dir <value>".
"#);
            Ok(())
        }
        1 => {
            let v = context.arguments.value(0)?;
            match v {
                Value::Command(cmd) =>
                    halp(cmd.help(), &context.printer),
                Value::Type(t) => halp(&t, &context.printer),
                v => halp(&v, &context.printer),
            }
            Ok(())
        }
        _ => argument_error("The help command expects at most one argument")
    }
}

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_lazy_namespace(
        "traversal",
        Box::new(move |env: &Scope| {
            env.declare_command(
                "ls", find::perform_ls, true,
                "ls @file:file", "Non-recursively list files", None)?;
            env.declare_command(
                "find", find::perform_find, true,
                "find @file:file",
                "Recursively list files", None)?;
            env.declare_command(
                "cd", cd, true,
                "cd directory:(file,string,glob)",
                "Change to the specified working directory", None)?;
            env.declare_command(
                "pwd", pwd, false,
                "pwd", "Return the current working directory", None)?;
            env.declare_command(
                "help", help, false,
                "help topic:any",
                "Show help about the specified thing",
                Some(r#"    Examples:

    help ls
    help integer
    help help"#))?;
            env.readonly();
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
