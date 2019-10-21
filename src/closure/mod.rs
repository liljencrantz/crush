use crate::job::JobDefinition;
use crate::env::Env;
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

    pub fn compile(&self, parent_env: &Env) -> Closure {
        Closure { jobs: self.jobs.clone(), parent_env: parent_env.clone() }
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
    parent_env: Env,
}

impl Closure {
    pub fn get_jobs(&self) -> &Vec<JobDefinition> {
        &self.jobs
    }
}

impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        false
    }
}
