use crate::lang::errors::CrushResult;

pub trait JobControl {
    fn terminate(&self) -> CrushResult<()>;
}

pub type JobController = Box<dyn JobControl + Send>;