use std::thread::{JoinHandle, ThreadId};
use crate::lang::printer::Printer;
use crate::lang::errors::{to_crush_error, CrushResult};
use std::sync::{Arc, Mutex};
use std::thread;
use crossbeam::channel::Sender;
use crossbeam::channel::Receiver;
use crossbeam::channel::unbounded;
use std::time::Duration;

struct ThreadStoreInternal {
    handles: Vec<JoinHandle<CrushResult<()>>>,
    sender: Sender<ThreadId>,
    receiver: Receiver<ThreadId>,
}

fn join_handle(handle: JoinHandle<CrushResult<()>>, printer: &Printer) {
    match handle.join() {
        Ok(res) => { printer.handle_error(res) }
        Err(_) => printer.error("Unknown error while waiting for command to exit"),
    }
}

#[derive(Clone)]
pub struct ThreadStore {
    data: Arc<Mutex<ThreadStoreInternal>>,
}

impl ThreadStore {
    pub fn new() -> ThreadStore {
        let (sender, receiver) = unbounded();

        ThreadStore {
            data: Arc::from(Mutex::new(ThreadStoreInternal {
                handles: Vec::new(),
                sender,
                receiver,
            })),
        }
    }

    fn exit(&self) {
        let mut data = self.data.lock().unwrap();
        let _ = data.sender.send(std::thread::current().id());
    }

    /**
    Spawn a new thread
    */
    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
        where
            F: FnOnce() -> CrushResult<()>,
            F: Send + 'static,
    {
        let slef = self.clone();
        let handle = to_crush_error(thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let res = f();
                slef.exit();
                res
            }))?;
        let id = handle.thread().id();
        let mut data = self.data.lock().unwrap();
        data.handles.push(handle);
        Ok(id)
    }

    /**
    Block calling thread until all other threads have exited
    */
    pub fn join(&self, printer: &Printer) {
        loop {
            let mut data = self.data.lock().unwrap();
            match data.handles.pop() {
                None => break,
                Some(h) => {
                    drop(data);
                    join_handle(h, printer);
                }
            }
        }
    }

    /**
    Error report all threads that have already exited
    */
    pub fn reap(&self, printer: &Printer) {
        let mut data = self.data.lock().unwrap();
        let mut kill_list = Vec::new();
        while let Ok(id) = data.receiver.recv_timeout(Duration::from_nanos(0)) {
            kill_list.push(id);
        }
        drop(data);
        for id in kill_list {
            self.join_one(id, printer);
        }
    }

    /**
    Block calling thread until specified thread has exited
    */
    pub fn join_one(&self, id: ThreadId, printer: &Printer) {
        let mut data = self.data.lock().unwrap();
        let mut kill_idx = None;
        for idx in 0..data.handles.len() {
            if data.handles[idx].thread().id() == id {
                kill_idx = Some(idx);
                break;
            }
        }
        if let Some(idx) = kill_idx {
            let h = data.handles.remove(idx);
            drop(data);
            join_handle(h, printer);
        }
    }
}
