use crate::lang::errors::CrushResult;
use crossbeam::channel::{Receiver, Sender};
use crossbeam::select;
use itertools::Either;
use std::thread::ThreadId;

pub trait JobControl {
    fn terminate(&self) -> CrushResult<()>;
    fn pause(&self) -> CrushResult<()>;
    fn resume(&self) -> CrushResult<()>;
}

pub type JobController = Box<dyn JobControl + Send>;

pub struct ChannelBasedController(Sender<StreamControlMessage>);

impl ChannelBasedController {
    pub fn new(sender: Sender<StreamControlMessage>) -> Self {
        ChannelBasedController(sender)
    }
}

impl JobControl for ChannelBasedController {
    fn terminate(&self) -> CrushResult<()> {
        Ok(self.0.send(StreamControlMessage::Terminate)?)
    }

    fn pause(&self) -> CrushResult<()> {
        Ok(self.0.send(StreamControlMessage::Pause)?)
    }

    fn resume(&self) -> CrushResult<()> {
        Ok(self.0.send(StreamControlMessage::Resume)?)
    }
}

pub enum StreamControlMessage {
    Terminate,
    Pause,
    Resume,
}

pub struct InterruptibleJoinHandle<T> {
    result_receiver: Receiver<T>,
    control_receiver: Receiver<StreamControlMessage>,
    id: ThreadId,
    name: Option<String>,
}

impl<T> InterruptibleJoinHandle<T> {
    pub fn new(
        id: ThreadId,
        name: Option<&str>,
        result_receiver: Receiver<T>,
        control_receiver: Receiver<StreamControlMessage>,
    ) -> Self {
        InterruptibleJoinHandle {
            id,
            name: name.map(|x| x.to_string()),
            result_receiver,
            control_receiver,
        }
    }

    pub fn id(&self) -> ThreadId {
        self.id
    }

    pub fn name(&self) -> &Option<String> {
        &self.name
    }

    pub fn join(&self) -> CrushResult<Either<T, StreamControlMessage>> {
        select! {
            recv(self.result_receiver) -> result => match result {
                Ok(res) => Ok(Either::Left(res)),
                Err(err) => Err(err.into()),
            },
            recv(self.control_receiver) -> control => match control {
                Ok(msg) => Ok(Either::Right(msg)),
                Err(err) => Err(err.into()),
            },
        }
    }
}
