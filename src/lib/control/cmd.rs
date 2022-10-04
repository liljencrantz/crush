use signature::signature;
use std::process::Stdio;
use std::io::{Read, Write};
use std::borrow::BorrowMut;
use std::path::PathBuf;
use crate::{argument_error_legacy, CrushResult, to_crush_error};
use crate::lang::argument::{Argument, SwitchStyle};
use crate::lang::errors::mandate;
use crate::lang::ordered_string_map::OrderedStringMap;
use crate::lang::value::Value;
use crate::lang::value::Value::{Binary, BinaryInputStream};
use crate::state::contexts::CommandContext;
use crate::lang::value::ValueType;
use crate::lang::command::OutputType::Known;
use crate::lang::command_invocation::resolve_external_command;
use crate::util::file::cwd;

#[signature(
cmd,
short = "Execute an external command",
long = "Globs are expanded. Argument and switch order is preserved.",
output = Known(ValueType::BinaryInputStream),
can_block = true,
)]
pub struct Cmd {
    command: PathBuf,
    #[named()]
    #[description("Switches to pass in to the command. The name will be prepended with a double dash '--', unless it is a single character name, in which case a single dash '-' will be prepended")]
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
        .filter(|s| { !s.is_empty() })
        .map(|s| { s.to_string() })
        .collect()
    )
}

fn cmd_internal(
    context: CommandContext,
    file: PathBuf,
    mut arguments: Vec<Argument>)
    -> CrushResult<()> {
    let use_tty = !context.input.is_pipeline() && !context.output.is_pipeline();
    let mut cmd = std::process::Command::new(file.as_os_str());

    for a in arguments.drain(..) {
        match a.argument_type {
            None => {
                match a.value {
                    Value::Glob(glob) => {
                        let mut files = Vec::new();
                        glob.glob_files(&cwd()?, &mut files)?;
                        for file in files {
                            cmd.arg(file);
                        }
                    }
                    _ => for s in format_value(&a.value)? {
                        cmd.arg(s);
                    }
                }
            }

            Some(name) => {
                let switch =
                    match a.switch_style {
                        SwitchStyle::None =>
                            if name.len() == 1 {
                                format!("-{}", name)
                            } else {
                                format!("--{}", name)
                            },
                        SwitchStyle::Single =>
                            format!("-{}", name),
                        SwitchStyle::Double =>
                            format!("--{}", name),
                    };
                match a.value {
                    Value::Bool(true) => {
                        cmd.arg(switch);
                    }
                    Value::Glob(glob) => {
                        let mut files = Vec::new();
                        glob.glob_files(&cwd()?, &mut files)?;
                        for file in files {
                            cmd.arg(format!("{}={}", switch, mandate(file.to_str(), "Invalid file name")?));
                        }
                    }
                    _ => {
                        for s in format_value(&a.value)? {
                            cmd.arg(format!("{}={}", switch, s));
                        }
                    }
                }
            }
        }
    }

    if use_tty {
        cmd
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        to_crush_error(to_crush_error(cmd.spawn())?.wait())?;
        Ok(())
    } else {
        let input = context.input.recv()?;

        let (stdout_reader, stdout_writer) = os_pipe::pipe().unwrap();
        let (mut stderr_reader, stderr_writer) = os_pipe::pipe().unwrap();

        cmd.stdin(Stdio::piped());
        cmd.stdout(stdout_writer);
        cmd.stderr(stderr_writer);

        let mut child = to_crush_error(cmd.spawn())?;
        let mut stdin = mandate(child.stdin.take(), "Expected stdin stream")?;

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
                    to_crush_error(std::io::copy(r.as_mut(), stdin.borrow_mut()))?;
                    Ok(())
                })?;
            }
            _ => return argument_error_legacy("Invalid input: Expected binary data"),
        }

        context.output.send(BinaryInputStream(Box::from(stdout_reader)))?;
        let my_context = context.clone();
        context.spawn("cmd:stderr", move || {
            let _ = &my_context;
            let mut buff = Vec::new();
            to_crush_error(stderr_reader.read_to_end(&mut buff))?;
            let errors = to_crush_error(String::from_utf8(buff))?;
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
        return argument_error_legacy("No command given");
    }
    match arguments.remove(0).value {
        Value::File(f) => {
            let file = if f.exists() {
                Some(f.to_path_buf())
            } else {
                resolve_external_command(mandate(f.to_str(), "Invalid command name")?, &context.scope)?
            };

            if let Some(file) = file {
                cmd_internal(context, file, arguments)
            } else {
                argument_error_legacy(format!("Unknown command {}", f.to_str().unwrap_or("<encoding error>")))
            }
        }
        Value::String(s) => {
            if let Some(file) = resolve_external_command(s.as_ref(), &context.scope)? {
                cmd_internal(context, file, arguments)
            } else {
                argument_error_legacy(format!("Unknown command {}", s))
            }
        }

        _ => argument_error_legacy("Not a valid command"),
    }
}
