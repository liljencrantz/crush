use std::io;

mod errors;
mod stream;
mod result;
mod commands;
mod state;
mod job;

use std::io::Write;

fn prompt() -> Option<String> {
    let mut cmd = String::new();
    print!("> ");
    io::stdout().flush();
    return match io::stdin().read_line(&mut cmd) {
        Ok(_) => Some(cmd),
        Err(_) => None,
    }
}

fn repl() {
    let mut state = state::State::new();
    loop {
        match prompt() {
            Some(cmd) => {
                let mut job = job::Job::new(&cmd);
                match job.compile(&state) {
                    Ok(_) => {
                        job.run(&state);
                        job.mutate(&mut state);
                    }
                    Err(_) => {
                        for err in job.compile_errors {
                            println!("Compiler error: {}", err.message);
                        }
                    }
                }
            },
            None => break,
        }
    }
}

fn main() {
    repl();
}
