use crate::job::JobDefinition;
use crate::state::State;
use std::sync::Arc;
use crate::namespace::Namespace;

#[derive(Clone)]
pub struct ClosureDefinition {
    jobs: Vec<JobDefinition>,
}

impl ClosureDefinition {
    pub fn new(jobs: Vec<JobDefinition>) -> ClosureDefinition {
        ClosureDefinition {
            jobs,
        }
    }

    pub fn compile(&self, parent_state: &State) -> Closure{
        Closure { jobs: self.jobs.clone(), parent_state: parent_state.clone() }
    }
}

impl PartialEq for ClosureDefinition {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}

#[derive(Clone)]
pub struct Closure {
    jobs: Vec<JobDefinition>,
    parent_state: State,
}

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
