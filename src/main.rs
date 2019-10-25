mod replace;
mod errors;
mod glob;
mod stream;
mod data;
mod commands;
mod namespace;
mod env;
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
use crate::errors::{JobError, JobResult};
use std::error::Error;
use std::sync::Arc;
use std::borrow::BorrowMut;
use crate::printer::Printer;
use std::sync::mpsc::channel;
use crate::stream::{streams, spawn_print_thread};
use crate::job::Job;
use crate::data::JobOutput;

fn repl() -> JobResult<()>{
    let mut global_env = env::Env::new();
    let printer =  Printer::new();

    add_builtins(&global_env)?;
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
                            let (first_output, first_input) = streams();
                            drop(first_output);
                            let (last_output, last_input) = streams();
                            match job_definition.compile(&global_env, &printer,&vec![], first_input, last_output) {
                                Ok(mut job) => {
                                    let handle = job.execute();
                                    spawn_print_thread(&printer, JobOutput { types: job.get_output_type().clone(), stream: last_input } );
                                    job.wait(handle, &printer);
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
