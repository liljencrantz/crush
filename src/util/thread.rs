use std::thread::JoinHandle;
use crate::lang::errors::CrushResult;
use std::thread;
use crate::lang::job::JobJoinHandle;
use crate::lang::printer::printer;

pub fn build(name: &str) -> thread::Builder {
    thread::Builder::new().name(name.to_string())
}

pub fn handle(h: Result<JoinHandle<()>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}
