use std::thread::{JoinHandle, Thread, ThreadId};
use crate::lang::printer::Printer;
use crate::lang::errors::{to_crush_error, CrushResult};
use std::sync::{Arc, Mutex};
use std::thread;
use std::any::Any;

struct ThreadData {
    handle: JoinHandle<CrushResult<()>>,
}

impl ThreadData {
    fn join(self, printer: &Printer) {
        match self.handle.join() {
            Ok(res) => {printer.handle_error(res)},
            Err(_) => printer.error("Unknown error while waiting for command to exit"),
        }
    }
}

#[derive(Clone)]
pub struct ThreadStore {
    data: Arc<Mutex<Vec<ThreadData>>>,
}

impl ThreadStore {

    pub fn new() -> ThreadStore {
        ThreadStore {
            data: Arc::from(Mutex::new(Vec::new())),
        }
    }

    pub fn spawn<F>(&self, name: &str, f: F) -> CrushResult<ThreadId>
        where
            F: FnOnce() -> CrushResult<()>,
            F: Send + 'static,
    {
        let handle = to_crush_error(thread::Builder::new()
            .name(name.to_string())
            .spawn(f))?;
        let id = handle.thread().id();
        let mut data = self.data.lock().unwrap();
        data.push(ThreadData {
            handle,
        });
        Ok(id)
    }

    pub fn join(&self, printer: &Printer) {
        loop {
            let mut data = self.data.lock().unwrap();
            match data.pop() {
                None => break,
                Some(h) => {
                    drop(data);
                    h.join(printer);
                },
            }
        }
    }

    pub fn join_one(&self, id: ThreadId, printer: &Printer) {
        let mut data = self.data.lock().unwrap();
        let mut kill_idx = None;
        for idx in 0..data.len() {
            if data[idx].handle.thread().id() == id {
                kill_idx = Some(idx);
                break;
            }
        }
        if let Some(idx) = kill_idx {
            let h = data.remove(idx);
            drop(data);
            h.join(printer);
        }
    }
}
