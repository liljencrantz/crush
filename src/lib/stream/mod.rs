use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;
use crate::lang::{value::Value};
use crate::lang::command::CrushCommand;

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
    env.declare("head", Value::Command(CrushCommand::command(head::perform, true)))?;
    env.declare("tail", Value::Command(CrushCommand::command(tail::perform, true)))?;
    env.declare("where", Value::Command(CrushCommand::command(r#where::perform, true)))?;
    env.declare("sort", Value::Command(CrushCommand::command(sort::perform, true)))?;
    env.declare("reverse", Value::Command(CrushCommand::command(reverse::perform, true)))?;
    env.declare("group", Value::Command(CrushCommand::command(group::perform, true)))?;
    env.declare("join", Value::Command(CrushCommand::command(join::perform, true)))?;
    env.declare("uniq", Value::Command(CrushCommand::command(uniq::perform, true)))?;
    //env.declare_str("aggr", Value::Command(CrushCommand::command(aggr::perform)))?;
    env.declare("count", Value::Command(CrushCommand::command(count::perform, true)))?;
    env.declare("sum", Value::Command(CrushCommand::command(sum::perform, true)))?;
    env.declare("select", Value::Command(CrushCommand::command(select::perform, true)))?;
    env.declare("enumerate", Value::Command(CrushCommand::command(enumerate::perform, true)))?;
    env.declare("zip", Value::Command(CrushCommand::command(zip::perform, true)))?;
    env.declare("seq", Value::Command(CrushCommand::command(seq::perform, true)))?;
    env.readonly();
    Ok(())
}
