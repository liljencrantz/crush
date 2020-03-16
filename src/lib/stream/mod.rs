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
    root.r#use(&env);
    env.declare("head", Value::Command(SimpleCommand::new(head::perform, true)))?;
    env.declare("tail", Value::Command(SimpleCommand::new(tail::perform, true)))?;
    env.declare("where", Value::Command(SimpleCommand::new(r#where::perform, true)))?;
    env.declare("sort", Value::Command(SimpleCommand::new(sort::perform, true)))?;
    env.declare("reverse", Value::Command(SimpleCommand::new(reverse::perform, true)))?;
    env.declare("group", Value::Command(SimpleCommand::new(group::perform, true)))?;
    env.declare("join", Value::Command(SimpleCommand::new(join::perform, true)))?;
    env.declare("uniq", Value::Command(SimpleCommand::new(uniq::perform, true)))?;
    //env.declare_str("aggr", Value::Command(SimpleCommand::new(aggr::perform)))?;
    env.declare("count", Value::Command(SimpleCommand::new(count::perform, true)))?;
    env.declare("sum", Value::Command(SimpleCommand::new(sum::perform, true)))?;
    env.declare("select", Value::Command(SimpleCommand::new(select::perform, true)))?;
    env.declare("enumerate", Value::Command(SimpleCommand::new(enumerate::perform, true)))?;
    env.declare("zip", Value::Command(SimpleCommand::new(zip::perform, true)))?;
    env.declare("seq", Value::Command(SimpleCommand::new(seq::perform, true)))?;
    env.readonly();
    Ok(())
}
