mod replace;
mod errors;
mod glob;
mod stream;
mod data;
mod commands;
mod namespace;
mod env;
mod job;
mod base_lexer;
mod lexer;
mod closure;
mod parser;
mod printer;
mod stream_printer;
mod format;
mod thread_util;

use crate::lexer::Lexer;

extern crate rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use commands::add_commands;
use crate::errors::{JobResult};
use std::error::Error;
use crate::printer::Printer;
use crate::stream::{empty_channel};
use crate::stream_printer::spawn_print_thread;
use crate::data::Cell;
use crate::env::get_home;
use std::path::Path;

fn crush_history_file() -> Box<str> {
    Box::from(
        get_home()
            .unwrap_or(Box::from(Path::new(".")))
            .join(Path::new(".crush_history"))
            .to_str()
            .unwrap_or(".crush_history"))
}

fn repl() -> JobResult<()> {
    let global_env = env::Env::new();
    let printer = Printer::new();

    add_commands(&global_env)?;
    global_env.declare_str("true", Cell::Bool(true))?;
    global_env.declare_str("false", Cell::Bool(false))?;
    global_env.declare_str("global", Cell::Env(global_env.clone()))?;

    let mut rl = Editor::<()>::new();
    rl.load_history(crush_history_file().as_ref());
    loop {
        let readline = rl.readline("crush> ");

        match readline {
            Ok(cmd) => {
                if !cmd.is_empty() {
                    rl.add_history_entry(cmd.as_str());
                    match parser::parse(&mut Lexer::new(&cmd)) {
                        Ok(jobs) => {
                            for job_definition in jobs {
                                let last_output = spawn_print_thread(&printer);
                                match job_definition.spawn_and_execute(&global_env, &printer, empty_channel(), last_output) {
                                    Ok(handle) => {
                                        handle.join(&printer);
                                    }
                                    Err(e) => printer.job_error(e),
                                }
                            }
                        }
                        Err(error) => {
                            printer.job_error(error);
                        }
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                printer.line("^C");
            }
            Err(ReadlineError::Eof) => {
                printer.line("exit");
                break;
            }
            Err(err) => {
                printer.line(err.description());
                break;
            }
        }
        match rl.save_history(crush_history_file().as_ref()) {
            Ok(_) => {}
            Err(_) => {
                printer.line("Error: Failed to save history.");
            }
        }
    }
    printer.shutdown();
    return Ok(());
}

fn main() {
    match repl() {
        Ok(_) => (),
        Err(e) => println!("Error during initialization: {}", e.message),
    }
}
