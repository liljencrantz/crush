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
use std::sync::Arc;
use std::borrow::BorrowMut;
use crate::printer::Printer;

fn repl() -> Result<(), JobError>{
    let mut state = state::State::new();
    let printer =  Printer::new();

    add_builtins(&state)?;
    let mut rl = Editor::<()>::new();
    rl.load_history(".crush_history").unwrap();
    loop {
        let readline = rl.readline("crush> ");

        match readline {
            Ok(cmd) => {
                rl.add_history_entry(cmd.as_str());
                match parser::parse(&mut Lexer::new(&cmd)) {
                    Ok(jobs) => {
                        for job_definition in jobs {
                            match job_definition.compile(&state) {
                                Ok(mut job) => {
                                    job.exec(&mut state, &printer);
                                    job.print(&printer);
                                    job.wait(&printer);
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
