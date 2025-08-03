use crate::lang::errors::CrushResult;
use crossbeam::channel::Sender;

pub trait JobControl {
    fn terminate(&self) -> CrushResult<()>;
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
        Ok(self.0.send(StreamControlMessage::Hangup)?)
    }
}

pub enum StreamControlMessage {
    Hangup,
}
