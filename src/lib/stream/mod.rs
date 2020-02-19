use crate::namespace::Namespace;
use crate::errors::CrushResult;
use crate::data::{Value, Command};

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

pub fn declare(root: &Namespace) -> CrushResult<()> {
    let env = root.create_namespace("stream")?;
    root.uses(&env);
    env.declare_str("head", Value::Command(Command::new(head::perform)))?;
    env.declare_str("tail", Value::Command(Command::new(tail::perform)))?;
    env.declare_str("where", Value::Command(Command::new(r#where::perform)))?;
    env.declare_str("sort", Value::Command(Command::new(sort::perform)))?;
    env.declare_str("reverse", Value::Command(Command::new(reverse::perform)))?;
    env.declare_str("group", Value::Command(Command::new(group::perform)))?;
    env.declare_str("join", Value::Command(Command::new(join::perform)))?;
    env.declare_str("uniq", Value::Command(Command::new(uniq::perform)))?;
//    env.declare_str("aggr", Value::Command(Command::new(aggr::perform)))?;
    env.declare_str("count", Value::Command(Command::new(count::perform)))?;
    env.declare_str("sum", Value::Command(Command::new(sum::perform)))?;
    env.declare_str("select", Value::Command(Command::new(select::perform)))?;
    env.declare_str("enumerate", Value::Command(Command::new(enumerate::perform)))?;
    env.declare_str("zip", Value::Command(Command::new(zip::perform)))?;
    Ok(())
}
