use crate::lang::errors::CrushResult;
use crossbeam::channel::Sender;

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
