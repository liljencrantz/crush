use std::thread::JoinHandle;
use crate::lang::errors::CrushResult;
use std::thread;
use crate::lang::job::JobJoinHandle;

pub fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

pub fn handle(h: Result<JoinHandle<CrushResult<()>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}
