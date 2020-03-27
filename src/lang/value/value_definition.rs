use regex::Regex;

use crate::{
    util::glob::Glob,
    lang::errors::{error, mandate, CrushResult},
    lang::scope::Scope,
    lang::value::Value,
    lang::job::JobJoinHandle,
    lang::stream::channels,
    lang::stream::empty_channel,
};
use crate::lang::{job::Job, argument::ArgumentDefinition, command::CrushCommand};
use crate::util::file::cwd;
use crate::lang::errors::block_error;
use crate::lang::command::Parameter;

#[derive(Clone)]
pub enum ValueDefinition {
    Value(Value),
    ClosureDefinition(Option<Vec<Parameter>>, Vec<Job>),
    JobDefinition(Job),
    Label(Box<str>),
    GetItem(Box<ValueDefinition>, Box<ValueDefinition>),
    GetAttr(Box<ValueDefinition>, Box<str>),
    Path(Box<ValueDefinition>, Box<str>),
}

fn file_get(f: &str) -> Option<Value> {
    let c = cwd();
    if c.is_err() {return None;}
    let p = c.unwrap().join(f);
        if p.exists() {
            Some(Value::File(p.into_boxed_path()))
        } else {
            None
        }
}

impl ValueDefinition {
    pub fn can_block(&self, arg: &Vec<ArgumentDefinition>, env: &Scope) -> bool {
        match self {
            ValueDefinition::JobDefinition(j) => j.can_block(env),
            ValueDefinition::GetItem(inner1, inner2) => inner1.can_block(arg, env) || inner2.can_block(arg, env),
            _ => false,
        }
    }

    pub fn compile_non_blocking(&self, env: &Scope) -> CrushResult<(Option<Value>, Value)> {
        let mut v = Vec::new();
        self.compile_internal(&mut v, env, false)
    }

    pub fn compile(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Scope) -> CrushResult<(Option<Value>, Value)> {
        self.compile_internal(dependencies, env, true)
    }

    pub fn compile_internal(&self, dependencies: &mut Vec<JobJoinHandle>, env: &Scope, can_block: bool) -> CrushResult<(Option<Value>, Value)> {
        Ok(match self {
            ValueDefinition::Value(v) => (None, v.clone()),
            ValueDefinition::JobDefinition(def) => {
                let first_input = empty_channel();
                let (last_output, last_input) = channels();
                if !can_block {
                    return block_error();
                }
                let j = def.invoke(&env, first_input, last_output)?;
                dependencies.push(j);
                (None, last_input.recv()?)
            }
            ValueDefinition::ClosureDefinition(p, c) => (None, Value::Command(CrushCommand::closure(p.clone(), c.clone(), env))),
            ValueDefinition::Label(s) =>
                (None, mandate(
                    env.get(s).or_else(|| file_get(s)),
                    format!("Unknown variable {}", self.to_string()).as_str())?),
            ValueDefinition::GetItem(c, i) => {
                let this = c.compile_internal(dependencies, env, can_block)?.1;
                let v = match (this.clone(), i.compile_internal(dependencies, env, can_block)?.1) {
                    (Value::File(s), Value::String(l)) =>
                        Value::File(s.join(l.as_ref()).into_boxed_path()),
                    (Value::List(list), Value::Integer(idx)) =>
                        list.get(idx as usize)?,
                    (Value::Dict(dict), c) =>
                        mandate(dict.get(&c), "Invalid subscript")?,
                    (Value::Scope(env), Value::String(name)) =>
                        mandate(env.get(name.as_ref()), "Invalid subscript")?,
                    (Value::Struct(row), Value::String(col)) =>
                        mandate(row.get(col.as_ref()), "Invalid subscript")?,
                    (Value::Struct(row), Value::Integer(idx)) =>
                        mandate(row.idx(idx as usize).map(|v| v.clone()), "Invalid subscript")?,
                    (Value::Table(o), Value::Integer(idx)) =>
                        Value::Struct(
                            mandate(o.rows().get(idx as usize), "Index out of range")?
                                .clone()
                                .into_struct(o.types())),
                    (Value::TableStream(o), Value::Integer(idx)) =>
                        Value::Struct(o.get(idx)?.into_struct(o.types())),
                    (parent, key) =>
                        return error(format!(
                            "Value of type {} can't be subscripted by key of type {}",
                            parent.value_type().to_string(),
                        key.value_type().to_string()).as_str()),
                };
                (Some(this), v)
            }

            ValueDefinition::GetAttr(parent_def, entry) => {
                let parent = parent_def.compile_internal(dependencies, env, can_block)?.1;
                let val = mandate(parent.field(&entry), format!("Missing field {}", entry).as_str())?;
                (Some(parent), val)
            }

            ValueDefinition::Path(parent_def, entry) => {
                let parent = parent_def.compile_internal(dependencies, env, can_block)?.1;
                let val = mandate(parent.path(&entry), format!("Missing path entry {}", entry).as_str())?;
                (Some(parent), val)
            }
        })
    }

    pub fn regex(s: &str, r: Regex) -> ValueDefinition {
        ValueDefinition::Value(Value::Regex(Box::from(s), r))
    }
}

impl ToString for ValueDefinition {
    fn to_string(&self) -> String {
        match &self {
            ValueDefinition::Value(v) => v.to_string(),
            ValueDefinition::Label(v) => v.to_string(),
            ValueDefinition::ClosureDefinition(s, c) => "<closure>".to_string(),
            ValueDefinition::JobDefinition(_) => "<job>".to_string(),
            ValueDefinition::GetItem(v, l) => format!("{}[{}]", v.to_string(), l.to_string()),
            ValueDefinition::GetAttr(v, l) => format!("{}:{}", v.to_string(), l),
            ValueDefinition::Path(v, l) => format!("{}/{}", v.to_string(), l),
        }
    }
}
