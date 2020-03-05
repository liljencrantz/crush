use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value, command::SimpleCommand};

mod head;
mod tail;
mod r#where;
mod sort;
mod reverse;

mod select;
mod enumerate;

mod uniq;
mod group;
mod join;
mod zip;
//mod aggr;

mod count;
mod sum;
mod seq;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("stream")?;
    root.uses(&env);
    env.declare_str("head", Value::Command(SimpleCommand::new(head::perform, true)))?;
    env.declare_str("tail", Value::Command(SimpleCommand::new(tail::perform, true)))?;
    env.declare_str("where", Value::Command(SimpleCommand::new(r#where::perform, true)))?;
    env.declare_str("sort", Value::Command(SimpleCommand::new(sort::perform, true)))?;
    env.declare_str("reverse", Value::Command(SimpleCommand::new(reverse::perform, true)))?;
    env.declare_str("group", Value::Command(SimpleCommand::new(group::perform, true)))?;
    env.declare_str("join", Value::Command(SimpleCommand::new(join::perform, true)))?;
    env.declare_str("uniq", Value::Command(SimpleCommand::new(uniq::perform, true)))?;
    //env.declare_str("aggr", Value::Command(SimpleCommand::new(aggr::perform)))?;
    env.declare_str("count", Value::Command(SimpleCommand::new(count::perform, true)))?;
    env.declare_str("sum", Value::Command(SimpleCommand::new(sum::perform, true)))?;
    env.declare_str("select", Value::Command(SimpleCommand::new(select::perform, true)))?;
    env.declare_str("enumerate", Value::Command(SimpleCommand::new(enumerate::perform, true)))?;
    env.declare_str("zip", Value::Command(SimpleCommand::new(zip::perform, true)))?;
    env.declare_str("seq", Value::Command(SimpleCommand::new(seq::perform, true)))?;
    env.readonly();
    Ok(())
}
