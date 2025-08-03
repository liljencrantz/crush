use std::fmt::{Display, Formatter};

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct JobId(usize);

impl From<usize> for JobId {
    fn from(value: usize) -> Self {
        JobId(value)
    }
}

impl From<JobId> for usize {
    fn from(value: JobId) -> usize {
        value.0
    }
}

impl Display for JobId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Hash)]
pub struct CommandId(usize);

impl CommandId {
    pub(crate) fn first() -> CommandId {
        CommandId(0)
    }

    pub fn next(&mut self) {
        self.0 = self.0 + 1;
    }
}

impl From<usize> for CommandId {
    fn from(value: usize) -> Self {
        CommandId(value)
    }
}

impl From<CommandId> for usize {
    fn from(value: CommandId) -> usize {
        value.0
    }
}
