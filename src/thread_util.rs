use std::thread::JoinHandle;
use crate::errors::CrushResult;
use std::thread;
use crate::lib::JobJoinHandle;

pub fn build(name: String) -> thread::Builder {
    thread::Builder::new().name(name)
}

pub fn handle(h: Result<JoinHandle<CrushResult<()>>, std::io::Error>) -> JobJoinHandle {
    JobJoinHandle::Async(h.unwrap())
}
