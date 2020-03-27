use std::thread::JoinHandle;
use std::thread;
use crate::lang::job::JobJoinHandle;

pub fn build(name: &str) -> thread::Builder {
    thread::Builder::new().name(name.to_string())
}

pub fn handle(h: Result<JoinHandle<()>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}
