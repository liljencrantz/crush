use std::io;
mod errors;
mod stream;
mod result;
mod commands;
mod state;
mod job;

fn repl() {
    loop {
        let mut cmd = String::new();
        let mut state = state::State::new();
        io::stdin().read_line(&mut cmd)
            .expect("Failed to read command");
        let mut job = job::Job::new(&cmd);
        job.compile(&state);
        job.run(&mut state);
    }
}

fn main() {
    repl();
}
