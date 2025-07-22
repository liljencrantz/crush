use crate::lang::ast::location::Location;
/// The definition of a value, as found in a Job.
use crate::lang::command::ParameterDefinition;
use crate::lang::pipe::black_hole;
use crate::lang::state::contexts::EvalContext;
use crate::lang::{command::CrushCommand, job::Job};
use crate::util::repr::Repr;
use crate::{
    lang::errors::CrushResult, lang::pipe::empty_channel, lang::pipe::pipe, lang::value::Value,
};
use std::fmt::{Display, Formatter, Pointer};
use crate::lang::ast::source::Source;

/// The definition of a value, as found in a Job.
#[derive(Clone)]
pub enum ValueDefinition {
    Value(Value, Source),
    ClosureDefinition {
        name: Option<Source>,
        signature: Option<Vec<ParameterDefinition>>,
        jobs: Vec<Job>,
        source: Source,
    },
    JobDefinition(Job),
    JobListDefinition(Vec<Job>),
    Identifier(Source),
    GetAttr(Box<ValueDefinition>, Source),
}

impl ValueDefinition {
    pub fn location(&self) -> Location {
        match self {
            ValueDefinition::Value(_, l) => l.location(),
            ValueDefinition::ClosureDefinition { source, .. } => source.location(),
            ValueDefinition::JobDefinition(j) => j.location(),
            ValueDefinition::Identifier(l) => l.location(),
            ValueDefinition::GetAttr(p, a) => p.location().union(a.location()),
            ValueDefinition::JobListDefinition(j) => j
                .last()
                .map(|j| j.location())
                .unwrap_or(Location::new(0, 0)),
        }
    }

    pub fn source(&self) -> &Source {
        match self {
            ValueDefinition::Identifier(source) 
            | ValueDefinition::GetAttr(_, source)
            | ValueDefinition::Value(_, source) 
            | ValueDefinition::ClosureDefinition { source, .. } => source,
             ValueDefinition::JobDefinition(j) => j.source(),
            ValueDefinition::JobListDefinition(j) => j
                .last()
                .map(|j| j.source())
                .unwrap(),
        }
    }

    pub fn can_block(&self, context: &mut EvalContext) -> bool {
        match self {
            ValueDefinition::JobDefinition(j) => j.can_block(context),
            ValueDefinition::GetAttr(_inner1, _inner2) => true,
            _ => false,
        }
    }

    pub fn eval_and_bind(&self, context: &mut EvalContext) -> CrushResult<Value> {
        let (t, v) = self.eval(context)?;
        Ok(t.map(|tt| v.clone().bind(tt)).unwrap_or(v))
    }

    pub fn eval(&self, context: &mut EvalContext) -> CrushResult<(Option<Value>, Value)> {
        Ok(match self {
            ValueDefinition::Value(v, _) => (None, v.clone()),
            ValueDefinition::JobDefinition(def) => {
                let first_input = empty_channel();
                let (last_output, last_input) = pipe();
                def.eval(context.job_context(first_input, last_output))?;
                (None, last_input.recv()?)
            }
            ValueDefinition::JobListDefinition(defs) => {
                for def in defs[..defs.len() - 1].iter() {
                    def.eval(context.job_context(empty_channel(), black_hole()))?;
                }
                let (last_output, last_input) = pipe();
                let last_def = &defs[defs.len() - 1];
                last_def.eval(context.job_context(empty_channel(), last_output))?;
                (None, last_input.recv()?)
            }

            ValueDefinition::ClosureDefinition {
                name,
                signature,
                jobs,
                source,
                ..
            } => (
                None,
                Value::Command(match signature {
                    None => <dyn CrushCommand>::closure_block(jobs.clone(), &context.env, source.clone()),
                    Some(signature) => <dyn CrushCommand>::closure_command(
                        name.clone(),
                        signature.clone(),
                        jobs.clone(),
                        &context.env,
                        &context.global_state,
                        source.clone(),
                    )?,
                }),
            ),
            ValueDefinition::Identifier(s) => (
                None,
                context
                    .env
                    .get(s.str())?
                    .ok_or(&format!("Unknown variable `{}`", s.str()))?,
            ),

            ValueDefinition::GetAttr(parent_def, entry) => {
                let (grand_parent, mut parent) = parent_def.eval(context)?;
                parent = if let Value::Command(parent_cmd) = &parent {
                    let first_input = empty_channel();
                    let (last_output, last_input) = pipe();
                    parent_cmd.eval(
                        context
                            .job_context(first_input, last_output)
                            .command_context(parent_def.source(), vec![], grand_parent),
                    )?;
                    last_input.recv()?
                } else {
                    parent
                };
                let val = parent.field(&entry.string())?.ok_or(&format!(
                    "Missing field `{}` in value of type `{}`",
                    entry.str(),
                    parent.value_type()
                ))?;
                (Some(parent), val)
            }
        })
    }
}

impl Display for ValueDefinition {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            ValueDefinition::Value(v, _location) => v.fmt(f),
            ValueDefinition::Identifier(v) => v.fmt(f),
            ValueDefinition::ClosureDefinition {
                signature, jobs, ..
            } => {
                f.write_str("{ ")?;
                if let Some(params) = signature {
                    f.write_str("| ")?;
                    for p in params {
                        p.fmt(f)?;
                        f.write_str(" ")?
                    }
                    f.write_str("| ")?;
                }

                for j in jobs {
                    j.fmt(f)?;
                    f.write_str(";\n")?;
                }
                f.write_str(" }")
            }
            ValueDefinition::JobDefinition(j) => j.fmt(f),
            ValueDefinition::GetAttr(v, l) => {
                std::fmt::Display::fmt(&v, f)?;
                f.write_str(":")?;
                l.fmt(f)
            }
            ValueDefinition::JobListDefinition(jl) => jl.fmt(f),
        }
    }
}

impl Repr for ValueDefinition {
    fn repr(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            ValueDefinition::Value(v, _location) => v.repr(f),
            ValueDefinition::Identifier(v) => {
                f.write_str("$")?;
                f.write_str(v.str())
            }
            ValueDefinition::ClosureDefinition {
                signature, jobs, ..
            } => {
                f.write_str("{ ")?;
                if let Some(params) = signature {
                    f.write_str("| ")?;
                    for p in params {
                        p.fmt(f)?;
                        f.write_str(" ")?
                    }
                    f.write_str("| ")?;
                }

                for j in jobs {
                    j.fmt(f)?;
                    f.write_str("; ")?;
                }
                f.write_str(" }")
            }
            ValueDefinition::JobDefinition(j) => {
                f.write_str("$(")?;
                j.fmt(f)?;
                f.write_str(")")
            }
            ValueDefinition::GetAttr(v, l) => {
                v.repr(f)?;
                f.write_str(":")?;
                l.fmt(f)
            }
            ValueDefinition::JobListDefinition(jl) => {
                f.write_str("$(")?;
                let mut first = true;
                for j in jl {
                    if !first {
                        f.write_str("; ")?;
                        first = false;
                    }
                    j.fmt(f)?;
                }
                f.write_str(")")
            }
        }
    }
}
