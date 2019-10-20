use crate::job::JobDefinition;
use crate::state::State;
use std::sync::Arc;
use crate::namespace::Namespace;

#[derive(Clone)]
pub struct Closure {
    jobs: Vec<JobDefinition>,
    parent_state: State,
}


impl Closure {
    pub fn new(jobs: Vec<JobDefinition>, parent_state: State) -> Closure {
        Closure {
            jobs,
            parent_state,
        }
    }
}

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
