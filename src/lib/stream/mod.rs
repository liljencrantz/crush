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
    env.declare("head", Value::Command(CrushCommand::command(
        head::perform, true,
        "head [lines:integer]", "Return the first lines of the input. Defaults to 10.", None)))?;
    env.declare("tail", Value::Command(CrushCommand::command(
        tail::perform, true,
        "tail [lines:integer]", "Return the last lines of the input. Defaults to 10.", None)))?;
    env.declare("where", Value::Command(CrushCommand::command_undocumented(r#where::perform, true)))?;
    env.declare("sort", Value::Command(CrushCommand::command_undocumented(sort::perform, true)))?;
    env.declare("reverse", Value::Command(CrushCommand::command(
        reverse::perform, true,
        "reverse", "Reverses the order of the rows in the input", None)))?;
    env.declare("group", Value::Command(CrushCommand::command_undocumented(group::perform, true)))?;
    env.declare("join", Value::Command(CrushCommand::command_undocumented(join::perform, true)))?;
    env.declare("uniq", Value::Command(CrushCommand::command_undocumented(uniq::perform, true)))?;
    //env.declare_str("aggr", Value::Command(CrushCommand::command_undocumented(aggr::perform)))?;
    env.declare("count", Value::Command(CrushCommand::command_undocumented(count::perform, true)))?;
    env.declare("sum", Value::Command(CrushCommand::command_undocumented(sum::perform, true)))?;
    env.declare("select", Value::Command(CrushCommand::command_undocumented(select::perform, true)))?;
    env.declare("enumerate", Value::Command(CrushCommand::command_undocumented(enumerate::perform, true)))?;
    env.declare("zip", Value::Command(CrushCommand::command_undocumented(zip::perform, true)))?;
    env.declare("seq", Value::Command(CrushCommand::command_undocumented(seq::perform, true)))?;
    env.readonly();
    Ok(())
}
