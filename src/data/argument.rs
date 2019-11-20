use crate::data::value::Value;
use crate::data::{ValueDefinition, ColumnType};
use crate::errors::{JobError, JobResult};
use crate::env::Env;
use crate::printer::Printer;
use crate::commands::{JobJoinHandle};

#[derive(Debug)]
pub struct BaseArgument<C> {
    pub name: Option<Box<str>>,
    pub value: C,
}

pub type ArgumentDefinition = BaseArgument<ValueDefinition>;

impl ArgumentDefinition {
    pub fn argument(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> Result<Argument, JobError> {
        Ok(Argument { name: self.name.clone(), value: self.value.compile(dependencies, env, printer)? })
    }
}

impl Clone for ArgumentDefinition {
    fn clone(&self) -> Self {
        ArgumentDefinition { name: self.name.clone(), value: self.value.clone() }
    }
}

pub type Argument = BaseArgument<Value>;

impl Argument {
    pub fn cell_type(&self) -> ColumnType {
        ColumnType { name: self.name.clone(), cell_type: self.value.value_type() }
    }
}

impl<C> BaseArgument<C> {
    pub fn named(name: &str, cell: C) -> BaseArgument<C> {
        BaseArgument {
            name: Some(Box::from(name)),
            value: cell,
        }
    }

    pub fn unnamed(cell: C) -> BaseArgument<C> {
        BaseArgument {
            name: None,
            value: cell,
        }
    }

    pub fn val_or_empty(&self) -> &str {
        self.name.as_ref().map(|v| v.as_ref()).unwrap_or("")
    }
}

pub trait ArgumentVecCompiler {
    fn compile(&self, dependencies: &mut Vec<JobJoinHandle>,  env: &Env, printer: &Printer) -> JobResult<Vec<Argument>>;
}

impl ArgumentVecCompiler for Vec<ArgumentDefinition> {
    fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Env, printer: &Printer) -> JobResult<Vec<Argument>> {
        self.iter()
            .map(|a| a.argument(dependencies, env, printer))
            .collect::<JobResult<Vec<Argument>>>()
    }
}
