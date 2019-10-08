mod errors;
mod glob;
mod stream;
mod cell;
mod commands;
mod state;
mod job;
mod lexer;
mod parser;

use job::Job;
use state::State;
use crate::lexer::Lexer;

extern crate rustyline;

use rustyline::error::ReadlineError;
use rustyline::Editor;

fn perform(job: &mut Job, state: &mut State) -> Result<(), ()> {
    job.spawn(state);
    job.wait();
    job.mutate(state)?;
    return Ok(());
}

fn repl() {
    let mut state = state::State::new();
    let mut rl = Editor::<()>::new();
    rl.load_history(".posh_history").unwrap();
    loop {
        let readline = rl.readline("posh> ");

        match readline {
            Ok(cmd) => {
                rl.add_history_entry(cmd.as_str());
                match parser::parse(&mut Lexer::new(&cmd), &state) {
                    Ok(jobs) => {
                        for mut job in jobs {
                            match perform(&mut job, &mut state) {
                                Ok(_) => {}
                                Err(_) => {
                                    for err in job.compile_errors {
                                        println!("Compiler error: {}", err.message);
                                    }
                                    for err in job.runtime_errors {
                                        println!("Runtime error: {}", err.message);
                                    }
                                }
                            }
                        }
                    }
                    Err(error) => {
                        println!("Compiler error: {}", error.message);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("^C");
            }
            Err(ReadlineError::Eof) => {
                println!("exit");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
        match rl.save_history(".posh_history") {
            Ok(_) => {}
            Err(_) => {
                println!("Error: Failed to save history.");
            }
        }
    }
}

fn main() {
    repl();
//      lexer::do_lex_test();
}
