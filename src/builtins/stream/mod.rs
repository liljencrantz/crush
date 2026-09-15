use crate::lang::command::CrushCommand;
use crate::lang::command::OutputType::Unknown;
use crate::lang::errors::CrushResult;
use crate::lang::state::scope::Scope;
use crate::lang::value::Value;

mod aggregation;
mod count;
mod drop;
mod each;
mod fold;
mod group;
mod head;
mod join;
mod predicate;
mod rename;
mod reverse;
mod sample;
mod select;
mod seq;
mod skip;
mod sort;
mod tail;
mod tee;
mod union;
mod uniq;
mod r#where;
mod zip;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let e = root.create_namespace(
        "stream",
        "Stream handling commands",
        Some(
            "Crush's pipes carry typed streams of rows, not bytes -- `stream` is where the \
             SQL-like operations on those streams live: filtering (`where`), sorting, \
             grouping, aggregating, joining two streams, deduplicating, and more. This \
             namespace is imported into the global scope, so every command here works equally \
             well as `stream:sort` or the bare word `sort` -- most Crush pipelines are built \
             by chaining these together with `|`."
                .to_string(),
        ),
        Box::new(move |env| {
            count::Count::declare(env)?;
            drop::Drop::declare(env)?;
            each::Each::declare(env)?;
            fold::Fold::declare(env)?;
            head::Head::declare(env)?;
            tail::Tail::declare(env)?;
            r#where::Where::declare(env)?;
            skip::Skip::declare(env)?;
            sort::Sort::declare(env)?;
            reverse::Reverse::declare(env)?;
            group::Group::declare(env)?;
            uniq::Uniq::declare(env)?;
            join::Join::declare(env)?;
            predicate::AnyMatch::declare(env)?;
            predicate::AllMatch::declare(env)?;
            sample::Sample::declare(env)?;
            tee::Tee::declare(env)?;
            aggregation::Sum::declare(env)?;
            aggregation::Avg::declare(env)?;
            aggregation::Median::declare(env)?;
            aggregation::Min::declare(env)?;
            aggregation::Max::declare(env)?;
            aggregation::Prod::declare(env)?;
            aggregation::Concat::declare(env)?;
            env.declare(
                "select",
                Value::Command(<dyn CrushCommand>::command(
                    select::select,
                    true,
                    ["stream", "select"],
                    "stream:select [copy_fields:string...] [*] [new_field=command]",
                    "Pass on some old fields and calculate new ones for each line of input",
                    Some(
                        r#"# Examples

    # Show only the filename and discard all other columns
    files | select file

    # Add an extra column to the output of files that shows the time passed since last modification
    files | select * age={(time.now() - modified)}"#,
                    ),
                    Unknown,
                    [],
                )),
            )?;
            seq::Seq::declare(env)?;
            zip::Zip::declare(env)?;
            union::Union::declare(env)?;
            rename::Rename::declare(env)?;
            Ok(())
        }),
    )?;
    root.r#use(&e);
    Ok(())
}
