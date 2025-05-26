pub mod rustyline_helper;

use std::fs;
use rustyline;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, EditMode};
use crate::util::file::home;
use std::path::PathBuf;
use crate::lang::ast::lexer::LexerMode;
use crate::lang::state::scope::Scope;
use crate::lang::pipe::{ValueSender, empty_channel, pipe, black_hole};
use crate::lang::errors::{CrushResult, data_error, error};
use crate::lang::execute;

use crate::lang::state::global_state::GlobalState;
use crate::lang::command::Command;
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::value::{ValueDefinition, Value};
use crate::lang::ast::location::Location;
use crate::lang::state::contexts::JobContext;

const DEFAULT_PROMPT: &'static str = "crush# ";

pub fn config_dir() -> CrushResult<PathBuf> {
    std::env::var("XDG_CONFIG_HOME")
        .map(|s| PathBuf::from(s).join("crush"))
        .or_else(|_| match home() {
            Ok(home) => Ok(home.join(".config/crush")),
            Err(e) => Err(e),
        })
}

fn crush_history_file() -> CrushResult<PathBuf> {
    Ok(config_dir()?.join("history"))
}

fn execute_command(
    prompt: Option<Command>,
    env: &Scope,
    global_state: &GlobalState,
) -> CrushResult<Option<String>> {
    match prompt {
        None => Ok(None),
        Some(prompt) => {
            let cmd = CommandInvocation::new(
                ValueDefinition::Value(Value::Command(prompt), Location::new(0, 0)),
                vec![]);
            let (snd, recv) = pipe();
            cmd.eval(JobContext::new(
                empty_channel(),
                snd,
                env.clone(),
                global_state.clone(),
            ))?;
            let v = recv.recv()?;
            match v {
                Value::String(s) => Ok(Some(s.to_string())),
                _ => data_error("Wrong output type of prompt command"),
            }
        }
    }
}

pub fn load_init(
    env: &Scope,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let file = config_dir()?.join("config.crush");
    if file.exists() {
        execute::file(env, &file, &black_hole(), global_state)
    } else {
        Ok(())
    }
}

pub fn run(
    global_env: Scope,
    pretty_printer: &ValueSender,
    global_state: &GlobalState,
) -> CrushResult<()> {
    let printer = global_state.printer().clone();
    printer.handle_error(load_init(&global_env, global_state));

    global_state.printer().line("Welcome to Crush");
    global_state.printer().line(r#"Type "help" for... help."#);

    let editor_config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .build();

    let h = rustyline_helper::RustylineHelper::new(
        global_state.clone(),
        global_env.clone(),
    );

    let mut editor = Editor::with_config(editor_config)?;
    editor.set_helper(Some(h));
    global_state.set_editor(Some(editor));

    if let Ok(file) = crush_history_file() {
        let _ = global_state.editor().as_mut().map(|rl| { rl.load_history(&file) });
    }
    loop {

        if let Ok(Some(title)) = execute_command(global_state.title(), &global_env, global_state) {
            println!("\x1b]0;{}\x07", title);
        }

        let prompt = match execute_command(global_state.prompt(), &global_env, global_state) {
            Ok(s) => s,
            Err(e) => {
                global_state.printer().crush_error(e);
                None
            }
        }.unwrap_or_else(|| DEFAULT_PROMPT.to_string());
        let readline = global_state.editor().as_mut().map(|rl| { rl.readline(&prompt) });

        match readline {
            Some(Ok(mut cmd)) =>
                if cmd.is_empty() {
                    global_state.threads().reap(global_state.printer())
                } else {

                    match (cmd.trim(), global_state.mode()) {
                        ("!!", _) => {
                            cmd = global_state
                                .editor().as_mut()
                                .map(|rl| { rl.history().into_iter().last().map(|s| {s.to_string()})})
                                .unwrap_or(None).unwrap_or(cmd);
                        }
                        ("(", LexerMode::Command) => {
                            global_state.set_mode(LexerMode::Expression);
                            continue;
                        }
                        (")", LexerMode::Expression) => {
                            global_state.set_mode(LexerMode::Command);
                            continue;
                        }
                        _ => {}
                    }
                    global_state.editor().as_mut().map(|rl| { rl.add_history_entry(&cmd) });
                    global_state.threads().reap(global_state.printer());
                    global_state
                        .printer()
                        .handle_error(
                            execute::string(
                                &global_env,
                                &cmd,
                                global_state.mode(),
                                pretty_printer,
                                global_state,
                            ));
                    global_state.threads().reap(global_state.printer());
                    if global_state.exit_status().is_some() {
                        break;
                    }
                    global_state.printer().ping();
                }
            Some(Err(ReadlineError::Interrupted)) => {
                global_state.printer().line("^C");
            }
            Some(Err(ReadlineError::Eof)) => {
                global_state.printer().line("exit");
                break;
            }
            Some(Err(err)) => {
                global_state.printer().handle_error::<()>(Err(err.into()));
                break;
            }
            None => {
                global_state.printer().line("no editor!");
                break;
            }
        }

        if let Ok(file) = crush_history_file() {
            if ensure_parent_exists(&file).is_ok() {
                match global_state.editor().as_mut().map(|rl| { rl.save_history(&file) }) {
                    Some(Err(err)) =>
                        global_state.printer().line(&format!(
                            "Error: Failed to save history to {}: {}",
                            file.as_os_str().to_str().unwrap_or("???"),
                            err)),
                    None => {
                        global_state.printer().line("no editor!");
                        break;
                    }
                    _ => {}
                }
            }
        }
    }

    if let Ok(file) = crush_history_file() {
        if let Some(Err(err)) = global_state.editor().as_mut().map(|rl| { rl.save_history(&file) }) {
            global_state.printer().line(&format!("Error: Failed to save history: {}", err))
        }
    }
    global_state.set_editor(None);
    Ok(())
}

fn ensure_parent_exists(file: &PathBuf) -> CrushResult<()> {
    if let Some(dir) = file.parent() {
        Ok(fs::create_dir_all(dir)?)
    } else {
        error("Missing parent directory")
    }
}
