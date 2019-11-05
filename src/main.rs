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
use crate::stream::{empty_stream};
use crate::stream_printer::spawn_print_thread;
use crate::data::Cell;

fn repl() -> JobResult<()> {
    let global_env = env::Env::new();
    let printer = Printer::new();

    add_commands(&global_env)?;
    global_env.declare("true", Cell::Bool(true))?;
    global_env.declare("false", Cell::Bool(false))?;

    let mut rl = Editor::<()>::new();
    rl.load_history(".crush_history").unwrap();
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
                                match job_definition.spawn_and_execute(&global_env, &printer, empty_stream(), last_output) {
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
        match rl.save_history(".crush_history") {
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
