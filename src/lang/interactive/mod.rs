mod rustyline_helper;

use rustyline;

use rustyline::error::ReadlineError;
use rustyline::{Editor, Config, CompletionType, EditMode, OutputStreamType};
use crate::util::file::home;
use std::path::PathBuf;
use crate::lang::data::scope::Scope;
use crate::lang::pipe::{ValueSender, empty_channel, pipe, black_hole};
use crate::lang::errors::{CrushResult, to_crush_error, data_error};
use crate::lang::execute;

use crate::lang::global_state::GlobalState;
use crate::lang::command::Command;
use crate::lang::command_invocation::CommandInvocation;
use crate::lang::value::{ValueDefinition, Value};
use crate::lang::ast::Location;
use crate::lang::execution_context::JobContext;

const DEFAULT_PROMPT: &'static str = "crush# ";

pub fn config_dir() -> CrushResult<PathBuf> {
    to_crush_error(std::env::var("XDG_CONFIG_HOME"))
        .map(|s| PathBuf::from(s).join("crush"))
        .or_else(|_| match home() {
            Ok(home) => Ok(home.join(".config/crush")),
            Err(e) => Err(e),
        })
}

fn crush_history_file() -> CrushResult<PathBuf> {
    Ok(config_dir()?.join("history"))
}

pub fn execute_prompt(
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
            cmd.invoke(JobContext::new(
                empty_channel(),
                snd,
                env.clone(),
                global_state.clone(),
            ))?;
            let v = recv.recv()?;
            match v {
                Value::String(s) => Ok(Some(s)),
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

    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .output_stream(OutputStreamType::Stdout)
        .build();

    let h = rustyline_helper::RustylineHelper::new(
        global_state.clone(),
        global_env.clone(),
    );

    let mut rl = Editor::with_config(config);
    rl.set_helper(Some(h));
    if let Ok(file) = crush_history_file() {
        let _ = rl.load_history(&file);
    }
    loop {
        let prompt = match execute_prompt(global_state.prompt(), &global_env, global_state) {
            Ok(s) => s,
            Err(e) => {
                global_state.printer().crush_error(e);
                None
            }
        }.unwrap_or_else(|| DEFAULT_PROMPT.to_string());
        let readline = rl.readline(&prompt);

        match readline {
            Ok(cmd) if cmd.is_empty() => global_state.threads().reap(global_state.printer()),
            Ok(cmd) => {
                rl.add_history_entry(&cmd);
                global_state.threads().reap(global_state.printer());
                global_state.printer().handle_error(
                    execute::string(
                        &global_env,
                        &cmd,
                        pretty_printer,
                        global_state,
                    ));
                global_state.threads().reap(global_state.printer());
                if global_state.exit_status().is_some() {
                    break;
                }
                global_state.printer().ping();
            }
            Err(ReadlineError::Interrupted) => {
                global_state.printer().line("^C");
            }
            Err(ReadlineError::Eof) => {
                global_state.printer().line("exit");
                break;
            }
            Err(err) => {
                global_state.printer().handle_error::<()>(to_crush_error(Err(err)));
                break;
            }
        }

        if let Ok(file) = crush_history_file() {
            if let Err(err) = rl.save_history(&file) {
                global_state.printer().line(&format!("Error: Failed to save history: {}", err))
            }
        }
    }
    if let Ok(file) = crush_history_file() {
        if let Err(err) = rl.save_history(&file) {
            global_state.printer().line(&format!("Error: Failed to save history: {}", err))
        }
    }
    Ok(())
}
