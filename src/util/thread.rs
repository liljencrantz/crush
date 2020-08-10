use crate::lang::job::JobJoinHandle;
use std::thread;
use std::thread::JoinHandle;

pub fn build(name: &str) -> thread::Builder {
    thread::Builder::new().name(name.to_string())
}

pub fn handle(h: Result<JoinHandle<()>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}
