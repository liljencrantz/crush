use crate::lang::argument::{Argument, SwitchStyle};
use crate::lang::command::OutputType::Known;
use crate::lang::command_invocation::resolve_external_command;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::value::Value;
use crate::lang::value::Value::{Binary, BinaryInputStream};
use crate::lang::value::ValueType;
use crate::state::contexts::CommandContext;
use crate::util::file::cwd;
use crate::CrushResult;
use signature::signature;
use std::borrow::BorrowMut;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::Stdio;
use crate::lang::errors::argument_error;

#[signature(
    control.cmd,
    short = "Execute an external command",
    long = "Globs are file-expanded. Argument and switch order is preserved.",
    output = Known(ValueType::BinaryInputStream),
    can_block = true,
)]
#[allow(unused)]
pub struct Cmd {
    #[description("The file path to the command to execute")]
    command: PathBuf,
    #[named()]
    #[description(
        "Switches to pass in to the command. The name will be prepended with a double dash '--', unless it is a single character name, in which case a single dash '-' will be prepended"
    )]
    switches: OrderedStringMap<Value>,
    #[unnamed()]
    #[description("Arguments to pass in to the command")]
    arguments: Vec<Value>,
}

fn format_value(v: &Value) -> CrushResult<Vec<String>> {
    Ok(v.clone()
        .materialize()?
        .to_string()
        .split("\n")
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

fn cmd_internal(
    context: CommandContext,
    file: PathBuf,
    mut arguments: Vec<Argument>,
) -> CrushResult<()> {
    let use_tty = !context.input.is_pipeline() && !context.output.is_pipeline();
    let mut cmd = std::process::Command::new(file.as_os_str());

    for a in arguments.drain(..) {
        match a.argument_type {
            None => match a.value {
                Value::Glob(glob) => {
                    let mut files = Vec::new();
                    glob.glob_files(&cwd()?, &mut files)?;
                    for file in files {
                        cmd.arg(file);
                    }
                }
                _ => {
                    for s in format_value(&a.value)? {
                        cmd.arg(s);
                    }
                }
            },

            Some(name) => {
                let (switch, join_string) = match a.switch_style {
                    SwitchStyle::None => {
                        if name.len() == 1 {
                            (format!("-{}", name), "")
                        } else {
                            (format!("--{}", name), "=")
                        }
                    }
                    SwitchStyle::Single => (format!("-{}", name), ""),
                    SwitchStyle::Double => (format!("--{}", name), "="),
                };
                match a.value {
                    Value::Bool(true) => {
                        cmd.arg(switch);
                    }
                    Value::Glob(glob) => {
                        let mut files = Vec::new();
                        glob.glob_files(&cwd()?, &mut files)?;
                        for file in files {
                            cmd.arg(format!(
                                "{}{}{}",
                                switch,
                                join_string,
                                file.to_str().ok_or("Invalid file name")?
                            ));
                        }
                    }
                    _ => {
                        for s in format_value(&a.value)? {
                            cmd.arg(format!("{}{}{}", switch, join_string, s));
                        }
                    }
                }
            }
        }
    }

    if use_tty {
        cmd.stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        cmd.spawn()?.wait()?;
        Ok(())
    } else {
        let input = context.input.recv()?;

        let (stdout_reader, stdout_writer) = os_pipe::pipe()?;
        let (mut stderr_reader, stderr_writer) = os_pipe::pipe()?;

        cmd.stdin(Stdio::piped());
        cmd.stdout(stdout_writer);
        cmd.stderr(stderr_writer);

        let mut child = cmd.spawn()?;
        let mut stdin = child.stdin.take().ok_or("Expected stdin stream")?;

        match input {
            Value::Empty => {
                drop(stdin);
            }
            Binary(v) => {
                context.spawn("cmd:stdin", move || {
                    stdin.write(&v)?;
                    Ok(())
                })?;
            }
            BinaryInputStream(mut r) => {
                context.spawn("cmd:stdin", move || {
                    std::io::copy(r.as_mut(), stdin.borrow_mut())?;
                    Ok(())
                })?;
            }
            _ => return argument_error("Invalid input: Expected binary data", &context.source),
        }

        context
            .output
            .send(BinaryInputStream(Box::from(stdout_reader)))?;
        let my_context = context.clone();
        context.spawn("cmd:stderr", move || {
            let _ = &my_context;
            let mut buff = Vec::new();
            stderr_reader.read_to_end(&mut buff)?;
            let errors = String::from_utf8(buff)?;
            for e in errors.split('\n') {
                let err = e.trim();
                if !err.is_empty() {
                    my_context.global_state.printer().error(err);
                }
            }
            Ok(())
        })?;

        child.wait()?;

        Ok(())
    }
}

fn cmd(mut context: CommandContext) -> CrushResult<()> {
    let mut arguments = context.remove_arguments();
    if arguments.is_empty() {
        return argument_error("No command given", &context.source);
    }
    match arguments.remove(0).value {
        Value::File(f) => {
            let file = if f.exists() {
                Some(f.to_path_buf())
            } else {
                resolve_external_command(f.to_str().ok_or("Invalid command name")?, &context.scope)?
            };

            if let Some(file) = file {
                cmd_internal(context, file, arguments)
            } else {
                argument_error(format!(
                    "Unknown command {}",
                    f.to_str().unwrap_or("<encoding error>")
                ), &context.source)
            }
        }
        Value::String(s) => {
            if let Some(file) = resolve_external_command(s.as_ref(), &context.scope)? {
                cmd_internal(context, file, arguments)
            } else {
                argument_error(format!("Unknown command `{}`", s), &context.source)
            }
        }

        _ => argument_error("Not a valid command", &context.source),
    }
}
