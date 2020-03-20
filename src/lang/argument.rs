use crate::lang::value::Value;
use crate::lang::{value::ValueDefinition, table::ColumnType};
use crate::lang::errors::{CrushError, CrushResult, error};
use crate::lang::scope::Scope;
use crate::lang::job::JobJoinHandle;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct BaseArgument<C: Clone> {
    pub name: Option<Box<str>>,
    pub value: C,
}

pub type ArgumentDefinition = BaseArgument<ValueDefinition>;

impl ArgumentDefinition {
    pub fn argument(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Scope) -> Result<Argument, CrushError> {
        Ok(Argument { name: self.name.clone(), value: self.value.compile(dependencies, env)?.1 })
    }

}
/*
impl Clone for ArgumentDefinition {
    fn clone(&self) -> Self {
        ArgumentDefinition { name: self.name.clone(), value: self.value.clone() }
    }
}
*/
pub type Argument = BaseArgument<Value>;

impl Argument {
/*    pub fn cell_type(&self) -> ColumnType {
        ColumnType { name: self.name.clone(), cell_type: self.value.value_type() }
    }*/
}

impl<C: Clone> BaseArgument<C> {
    pub fn unnamed_value(&self) -> CrushResult<C> {
        if self.name.is_some() {
            error("Expected an unnamed argument")
        } else {
            Ok(self.value.clone())
        }
    }

    pub fn new(name: Option<Box<str>>, value: C) -> BaseArgument<C> {
        BaseArgument {
            name,
            value,
        }
    }

    pub fn named(name: &str, value: C) -> BaseArgument<C> {
        BaseArgument {
            name: Some(Box::from(name)),
            value,
        }
    }

    pub fn unnamed(value: C) -> BaseArgument<C> {
        BaseArgument {
            name: None,
            value,
        }
    }

    pub fn val_or_empty(&self) -> &str {
        self.name.as_ref().map(|v| v.as_ref()).unwrap_or("")
    }
}

pub trait ArgumentVecCompiler {
    fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Scope) -> CrushResult<Vec<Argument>>;
}

impl ArgumentVecCompiler for Vec<ArgumentDefinition> {
    fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Scope) -> CrushResult<Vec<Argument>> {
        self.iter()
            .map(|a| a.argument(dependencies, env))
            .collect::<CrushResult<Vec<Argument>>>()
    }
}

pub fn column_names(arguments: &Vec<Argument>) -> Vec<Box<str>> {
    let mut taken = HashSet::new();
    taken.insert(Box::from("_"));
    let mut res = Vec::new();
    let mut tmp = String::new();
    for arg in arguments {
        let mut name = match &arg.name {
            None => "_",
            Some(name) => name.as_ref(),
        };
        if taken.contains(&Box::from(name)) {
            let mut idx = 1;
            tmp.truncate(0);
            tmp.push_str(name);
            loop {
                tmp.push_str(idx.to_string().as_str());
                if !taken.contains(tmp.as_str()) {
                    name = tmp.as_str();
                    break;
                }
                tmp.truncate(name.len());
            }
        }
        taken.insert(Box::from(name));
        res.push(Box::from(name));
    }

    res
}
