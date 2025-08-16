use crate::lang::errors::CrushResult;
use crate::lang::job_control::{ChannelBasedController, InterruptibleJoinHandle, StreamControlMessage};
use crate::lang::printer::Printer;
use crate::lang::state::handles::{CommandHandle};
use chrono::{DateTime, Local};
use crossbeam::channel::Sender;
use crossbeam::channel::unbounded;
use crossbeam::channel::{Receiver, bounded};
use itertools::Either;
use std::sync::{Arc, Mutex};
use std::thread;
use std::thread::{ThreadId};
use std::time::Duration;
use crate::lang::state::id::{CommandId, JobId};

/**
A thread management utility. Spawn, track and join on threads.
*/
#[allow(dead_code)] // Command is never read but is needed for resource tracking
struct ThreadData {
    handle: InterruptibleJoinHandle<CrushResult<()>>,
    creation_time: DateTime<Local>,
    command: CommandHandle,
    job_id: JobId,
    command_id: CommandId,
}

struct ThreadStoreInternal {
    threads: Vec<ThreadData>,
    sender: Sender<ThreadId>,
    receiver: Receiver<ThreadId>,
}

pub struct ThreadDescription {
    pub name: String,
    pub creation_time: DateTime<Local>,
    pub job_id: JobId,
    pub command_id: CommandId,
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
                threads: Vec::new(),
                sender,
                receiver,
            })),
        }
    }

    fn exit(&self) {
        let data = self.data.lock().unwrap();
        let _ = data.sender.send(std::thread::current().id());
    }

    /**
    Spawn a new thread
    */
    pub fn spawn<F>(
        &self,
        name: &str,
        command: &CommandHandle,
        f: F,
    ) -> CrushResult<ThreadId>
    where
        F: FnOnce() -> CrushResult<()>,
        F: Send + 'static,
    {
        let slef = self.clone();

        let (control_sender, control_receiver) = unbounded();
        let (result_sender, result_receiver) = bounded(1);

        command.register(Box::from(ChannelBasedController::new(control_sender)));

        let handle = thread::Builder::new()
            .name(name.to_string())
            .spawn(move || {
                let res = f();
                slef.exit();
                result_sender.send(res)
            })?;
        let id = handle.thread().id();
        let handle2 = InterruptibleJoinHandle::new(
            id,
            handle.thread().name(),
            result_receiver,
            control_receiver,
        );
        let mut data = self.data.lock().unwrap();
        data.threads.push(ThreadData {
            handle: handle2,
            creation_time: Local::now(),
            command: command.clone(),
            job_id: command.job_handle.id(),
            command_id: command.id,
        });
        Ok(id)
    }

    /**
    Block calling thread until all other threads have exited
    */
    pub fn join(&self, printer: &Printer) {
        loop {
            let mut data = self.data.lock().unwrap();
            match data.threads.pop() {
                None => break,
                Some(h) => {
                    drop(data);
                    printer.handle_error(h.handle.join());
                }
            }
        }
    }

    /**
    Error report all threads that have already exited
    */
    pub fn reap(&self, printer: &Printer) {
        let data = self.data.lock().unwrap();
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
        let mut thread_idx = None;
        for idx in 0..data.threads.len() {
            if data.threads[idx].handle.id() == id {
                thread_idx = Some(idx);
                break;
            }
        }
        if let Some(idx) = thread_idx {
            let h = data.threads.remove(idx);
            drop(data);

            match h.handle.join() {
                Ok(Either::Left(_)) => {}
                Ok(Either::Right(m)) => {
                    match m {
                        StreamControlMessage::Terminate => {}
                        StreamControlMessage::Pause => {
                            let mut data = self.data.lock().unwrap();
                            data.threads.push(h);
                        }
                        StreamControlMessage::Resume => {}
                    }
                }
                Err(err) => printer.crush_error(err), 
            }
        }
    }

    pub fn current_threads(&self) -> CrushResult<Vec<ThreadDescription>> {
        let data = self.data.lock().unwrap();
        let res = Ok(data
            .threads
            .iter()
            .map(|t| ThreadDescription {
                name: t.handle.name().clone().unwrap_or("<unnamed>".to_string()),
                creation_time: t.creation_time.clone(),
                job_id: t.job_id,
                command_id: t.command_id,
            })
            .collect());
        res
    }
}
