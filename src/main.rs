use std::io;
mod stream;
mod result;
mod job;

fn repl() {
    loop {
        let mut cmd = String::new();
        io::stdin().read_line(&mut cmd)
            .expect("Failed to read command");
        let mut job = job::Job::new(&cmd);
        job.compile();
        println!("Job: {}", job.to_string());
        let mut result = result::Result::new();
        job.run(&mut result);
        println!("Result: {}", result.to_string());
    }
}

fn main() {
    repl();
}
