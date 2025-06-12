use crate::lang::command::CrushCommand;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::CrushResult;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;

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
mod skip;
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
            skip::Skip::declare(env)?;
            sort::Sort::declare(env)?;
            reverse::Reverse::declare(env)?;
            group::Group::declare(env)?;
            uniq::Uniq::declare(env)?;
            join::Join::declare(env)?;
            aggregation::Sum::declare(env)?;
            aggregation::Avg::declare(env)?;
            aggregation::Median::declare(env)?;
            aggregation::Min::declare(env)?;
            aggregation::Max::declare(env)?;
            aggregation::Prod::declare(env)?;
            aggregation::First::declare(env)?;
            aggregation::Last::declare(env)?;
            aggregation::Concat::declare(env)?;
            env.declare(
                "select",
                Value::Command(<dyn CrushCommand>::command(
                    select::select,
                    true,
                    ["stream", "select"],
                    "stream:select [copy_fields:string...] [*] [new_field=command]",
                    "Pass on some old fields and calculate new ones for each line of input",
                    Some(r#"# Examples

    # Show only the filename and discard all other columns
    files | select file

    # Add an extra column to the output of files that shows the time passed since last modification
    files | select * age={(time.now() - modified)}"#),
                    Unknown,
                    [],
                )))?;
            seq::Seq::declare(env)?;
            zip::Zip::declare(env)?;
            Ok(())
        }))?;
    root.r#use(&e);
    Ok(())
}
