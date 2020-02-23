use crate::scope::Scope;
use crate::errors::CrushResult;
use crate::lang::{Value, SimpleCommand};

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

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("stream")?;
    root.uses(&env);
    env.declare_str("head", Value::Command(SimpleCommand::new(head::perform)))?;
    env.declare_str("tail", Value::Command(SimpleCommand::new(tail::perform)))?;
    env.declare_str("where", Value::Command(SimpleCommand::new(r#where::perform)))?;
    env.declare_str("sort", Value::Command(SimpleCommand::new(sort::perform)))?;
    env.declare_str("reverse", Value::Command(SimpleCommand::new(reverse::perform)))?;
    env.declare_str("group", Value::Command(SimpleCommand::new(group::perform)))?;
    env.declare_str("join", Value::Command(SimpleCommand::new(join::perform)))?;
    env.declare_str("uniq", Value::Command(SimpleCommand::new(uniq::perform)))?;
//    env.declare_str("aggr", Value::Command(Command::new(aggr::perform)))?;
    env.declare_str("count", Value::Command(SimpleCommand::new(count::perform)))?;
    env.declare_str("sum", Value::Command(SimpleCommand::new(sum::perform)))?;
    env.declare_str("select", Value::Command(SimpleCommand::new(select::perform)))?;
    env.declare_str("enumerate", Value::Command(SimpleCommand::new(enumerate::perform)))?;
    env.declare_str("zip", Value::Command(SimpleCommand::new(zip::perform)))?;
    env.readonly();
    Ok(())
}
