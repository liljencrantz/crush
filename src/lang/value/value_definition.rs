use crate::{
    lang::errors::CrushResult,
    lang::value::Value,
    lang::stream::channels,
    lang::stream::empty_channel,
};
use crate::lang::{job::Job, argument::ArgumentDefinition, command::CrushCommand};
use crate::lang::errors::{block_error, mandate};
use crate::lang::execution_context::CompileContext;
use std::path::PathBuf;
use crate::lang::command::Parameter;

#[derive(Clone)]
pub enum ValueDefinition {
    Value(Value),
    ClosureDefinition(Option<Box<str>>, Option<Vec<Parameter>>, Vec<Job>),
    JobDefinition(Job),
    Label(Box<str>),
    GetAttr(Box<ValueDefinition>, Box<str>),
    Path(Box<ValueDefinition>, Box<str>),
}

fn file_get(f: &str) -> Option<Value> {
    let p = PathBuf::from(f);
    if p.exists() {
        Some(Value::File(p))
    } else {
        None
    }
}

impl ValueDefinition {
    pub fn can_block(&self, _arg: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        match self {
            ValueDefinition::JobDefinition(j) => j.can_block(context),
            ValueDefinition::GetAttr(_inner1, _inner2) => true,
            _ => false,
        }
    }

    pub fn compile_unbound(&self, context: &mut CompileContext) -> CrushResult<(Option<Value>, Value)> {
        self.compile_internal(context, true)
    }

    pub fn compile_bound(&self, context: &mut CompileContext) -> CrushResult<Value> {
        let (t,v) = self.compile_internal(context, true)?;

        Ok(t.map(|tt| v.clone().bind(tt)).unwrap_or(v))
    }

    pub fn compile_internal(&self, context: &mut CompileContext, can_block: bool) -> CrushResult<(Option<Value>, Value)> {
        Ok(match self {
            ValueDefinition::Value(v) => (None, v.clone()),
            ValueDefinition::JobDefinition(def) => {
                let first_input = empty_channel();
                let (last_output, last_input) = channels();
                if !can_block {
                    return block_error();
                }
                let j = def.invoke(context.job_context(first_input, last_output))?;
                context.dependencies.push(j);
                (None, last_input.recv()?)
            }
            ValueDefinition::ClosureDefinition(name, p, c) =>
                (None, Value::Command(CrushCommand::closure(name.clone(), p.clone(), c.clone(), &context.env))),
            ValueDefinition::Label(s) =>
                (None, mandate(
                    context.env.get(s)?.or_else(|| file_get(s)),
                    format!("Unknown variable {}", self.to_string()).as_str())?),

            ValueDefinition::GetAttr(parent_def, entry) => {
                let parent = parent_def.compile_internal(context, can_block)?.1;
                let val = mandate(
                    parent.field(&entry)?,
                    format!("Missing field {} in value of type {}", entry, parent.value_type().to_string()).as_str())?;
                (Some(parent), val)
            }

            ValueDefinition::Path(parent_def, entry) => {
                let parent = parent_def.compile_internal(context, can_block)?.1;
                let val = mandate(parent.path(&entry), format!("Missing path entry {} in {}", entry, parent_def.to_string()).as_str())?;
                (Some(parent), val)
            }
        })
    }
}

impl ToString for ValueDefinition {
    fn to_string(&self) -> String {
        match &self {
            ValueDefinition::Value(v) => v.to_string(),
            ValueDefinition::Label(v) => v.to_string(),
            ValueDefinition::ClosureDefinition(_, _, _) => "<closure>".to_string(),
            ValueDefinition::JobDefinition(_) => "<job>".to_string(),
            ValueDefinition::GetAttr(v, l) => format!("{}:{}", v.to_string(), l),
            ValueDefinition::Path(v, l) => format!("{}/{}", v.to_string(), l),
        }
    }
}
