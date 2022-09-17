use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::CrushResult;
use crate::lang::state::scope::Scope;

mod count;
mod drop;
mod each;
mod enumerate;
mod group;
mod head;
mod join;
mod reverse;
mod select;
mod seq;
mod sort;
mod aggregation;
mod tail;
mod uniq;
mod r#where;
mod zip;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "stream",
        "Stream handling commands",
        Box::new(move |env| {
            count::Count::declare(env)?;
            drop::Drop::declare(env)?;
            each::Each::declare(env)?;
            enumerate::Enumerate::declare(env)?;
            head::Head::declare(env)?;
            tail::Tail::declare(env)?;
            r#where::Where::declare(env)?;
            sort::Sort::declare(env)?;
            reverse::Reverse::declare(env)?;
            group::Group::declare(env)?;
            uniq::Uniq::declare(env)?;
            env.declare_command(
                "join", join::join, true,
                "join left:field right:field", "Join two streams together on the specified keys",
                example!("join pid=(host:procs) pid=(host:threads| group pid tid={list:collect tid})"),
                Unknown,
                vec![],
            )?;
            aggregation::Sum::declare(env)?;
            aggregation::Avg::declare(env)?;
            aggregation::Min::declare(env)?;
            aggregation::Max::declare(env)?;
            aggregation::Mul::declare(env)?;
            aggregation::First::declare(env)?;
            aggregation::Last::declare(env)?;
            env.declare_command(
                "select", select::select, true,
                "select copy_fields:field... [%] new_field=definition:command",
                "Pass on some old fields and calculate new ones for each line of input",
                example!(r#"files | select user path={"{}/{}":format (pwd) file}"#), Unknown,
                vec![],
            )?;
            seq::Seq::declare(env)?;
            zip::Zip::declare(env)?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
