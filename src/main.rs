mod errors;
mod glob;
mod stream;
mod cell;
mod commands;
mod namespace;
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
use commands::add_builtins;

fn repl() {
    let mut state = state::State::new();
    add_builtins(&mut state.namespace);
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
                            job.exec(&mut state);
                            job.print();
                            job.wait();
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
