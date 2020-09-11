use crate::lang::command::Parameter;
use crate::lang::errors::{block_error, mandate};
use crate::lang::execution_context::CompileContext;
use crate::lang::{argument::ArgumentDefinition, command::CrushCommand, job::Job};
use crate::{
    lang::errors::CrushResult, lang::stream::channels, lang::stream::empty_channel,
    lang::value::Value,
};
use std::path::PathBuf;
use std::fmt::{Display, Formatter};
use crate::lang::ast::{Location, TrackedString};

#[derive(Clone)]
pub enum ValueDefinition {
    Value(Value, Location),
    ClosureDefinition(Option<TrackedString>, Option<Vec<Parameter>>, Vec<Job>, Location),
    JobDefinition(Job),
    Label(TrackedString),
    GetAttr(Box<ValueDefinition>, TrackedString),
    Path(Box<ValueDefinition>, TrackedString),
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
    pub fn location(&self) -> Location {
        match self {
            ValueDefinition::Value(_, l) => *l,
            ValueDefinition::ClosureDefinition(_, _, _, l) => *l,
            ValueDefinition::JobDefinition(j) => j.location(),
            ValueDefinition::Label(l) => l.location,
            ValueDefinition::GetAttr(p, a) |
            ValueDefinition::Path(p, a)=> p.location().union(a.location),
        }
    }

    pub fn can_block(&self, _arg: &[ArgumentDefinition], context: &mut CompileContext) -> bool {
        match self {
            ValueDefinition::JobDefinition(j) => j.can_block(context),
            ValueDefinition::GetAttr(_inner1, _inner2) => true,
            _ => false,
        }
    }

    pub fn compile_unbound(
        &self,
        context: &mut CompileContext,
    ) -> CrushResult<(Option<Value>, Value)> {
        self.compile_internal(context, true)
    }

    pub fn compile_bound(&self, context: &mut CompileContext) -> CrushResult<Value> {
        let (t, v) = self.compile_internal(context, true)?;

        Ok(t.map(|tt| v.clone().bind(tt)).unwrap_or(v))
    }

    pub fn compile_internal(
        &self,
        context: &mut CompileContext,
        can_block: bool,
    ) -> CrushResult<(Option<Value>, Value)> {
        Ok(match self {
            ValueDefinition::Value(v, _) => (None, v.clone()),
            ValueDefinition::JobDefinition(def) => {
                let first_input = empty_channel();
                let (last_output, last_input) = channels();
                if !can_block {
                    return block_error();
                }
                def.invoke(context.job_context(first_input, last_output))?;
                (None, last_input.recv()?)
            }
            ValueDefinition::ClosureDefinition(name, p, c, _) => (
                None,
                Value::Command(CrushCommand::closure(
                    name.clone(),
                    p.clone(),
                    c.clone(),
                    &context.env,
                    vec![],
                )),
            ),
            ValueDefinition::Label(s) => (
                None,
                mandate(
                    context.env.get(&s.string)?.or_else(|| file_get(&s.string)),
                    &format!("Unknown variable {}", self),
                )?,
            ),

            ValueDefinition::GetAttr(parent_def, entry) => {
                let (grand_parent, mut parent) = parent_def.compile_internal(context, can_block)?;
                parent = if let Value::Command(parent_cmd) = &parent {
                    if !can_block {
                        return block_error();
                    }
                    let first_input = empty_channel();
                    let (last_output, last_input) = channels();
                    parent_cmd.invoke(
                        context
                            .job_context(first_input, last_output)
                            .command_context(vec![], grand_parent),
                    )?;
                    last_input.recv()?
                } else {
                    parent
                };
                let val = mandate(
                    parent.field(&entry.string)?,
                    &format!(
                        "Missing field {} in value of type {}",
                        entry,
                        parent.value_type()
                    ),
                )?;
                (Some(parent), val)
            }

            ValueDefinition::Path(parent_def, entry) => {
                let parent = parent_def.compile_internal(context, can_block)?.1;
                let val = mandate(
                    parent.path(&entry.string),
                    &format!("Missing path entry {} in {}", entry, parent_def),
                )?;
                (Some(parent), val)
            }
        })
    }
}

impl Display for ValueDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            ValueDefinition::Value(v, _location) => v.fmt(f),
            ValueDefinition::Label(v) => v.fmt(f),
            ValueDefinition::ClosureDefinition(_, _, _, _location) => f.write_str("<closure>"),
            ValueDefinition::JobDefinition(_) => f.write_str("<job>"),
            ValueDefinition::GetAttr(v, l) => {
                v.fmt(f)?;
                f.write_str(":")?;
                l.fmt(f)
            }
            ValueDefinition::Path(v, l) => {
                v.fmt(f)?;
                f.write_str("/")?;
                l.fmt(f)
            }
        }
    }
}
