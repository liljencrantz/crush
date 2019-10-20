mod replace;
mod errors;
mod glob;
mod stream;
mod data;
mod commands;
mod namespace;
mod state;
mod job;
mod lexer;
mod closure;
mod parser;
mod printer;

use crate::lexer::Lexer;

extern crate rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;
use commands::add_builtins;
use crate::errors::JobError;
use std::error::Error;

fn repl() -> Result<(), JobError>{
    let mut state = state::State::new();
    add_builtins(&mut state.namespace)?;
    let mut rl = Editor::<()>::new();
    rl.load_history(".crush_history").unwrap();
    loop {
        let readline = rl.readline("crush> ");

        match readline {
            Ok(cmd) => {
                rl.add_history_entry(cmd.as_str());
                match parser::parse(&mut Lexer::new(&cmd), &state) {
                    Ok(jobs) => {
                        for job_definition in jobs {
                            let mut job = job_definition.job();
                            job.exec(&mut state);
                            job.print(&state.printer);
                            job.wait(&state.printer);
                        }
                    }
                    Err(error) => {
                        state.printer.job_error(error);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                state.printer.line("^C");
            }
            Err(ReadlineError::Eof) => {
                state.printer.line("exit");
                break;
            }
            Err(err) => {
                state.printer.line(err.description());
                break;
            }
        }
        match rl.save_history(".crush_history") {
            Ok(_) => {}
            Err(_) => {
                state.printer.line("Error: Failed to save history.");
            }
        }
    }
    state.printer.shutdown();
    return Ok(());
}

fn main() {
    match repl() {
        Ok(_) => (),
        Err(e) => println!("Error during initialization: {}", e.message),
    }
}
