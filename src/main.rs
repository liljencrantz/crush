mod errors;
mod stream;
mod result;
mod commands;
mod state;
mod job;
mod lexer;

use std::io;
use std::io::Write;

use job::Job;
use state::State;

fn prompt() -> Option<String> {
    let mut cmd = String::new();
    print!("> ");
    io::stdout().flush();
    return match io::stdin().read_line(&mut cmd) {
        Ok(_) => Some(cmd),
        Err(_) => None,
    };
}

fn perform(job: &mut Job, state: &mut State) -> Result<(), ()> {
    job.compile(state)?;
    job.run(state)?;
    job.mutate(state)?;
    return Ok(());
}

fn repl() {
    let mut state = state::State::new();
    loop {
        match prompt() {
            Some(cmd) => {
                let mut job = job::Job::new(&cmd);
                match perform(&mut job, &mut state) {
                    Ok(_) => {
                    }
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
            None => break,
        }
    }
}

fn main() {
    repl();
}
