use crate::lang::scope::Scope;
use crate::lang::errors::CrushResult;

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
mod sum_avg;
mod seq;

pub fn declare(root: &Scope) -> CrushResult<()> {
    let env = root.create_namespace("stream")?;
    root.r#use(&env);
    env.declare_command(
        "head",head::perform, true,
        "head [lines:integer]", "Return the first lines of the input. Defaults to 10.", None)?;
    env.declare_command(
        "tail",tail::perform, true,
        "tail [lines:integer]", "Return the last lines of the input. Defaults to 10.", None)?;
    env.declare_command(
        "where", r#where::r#where, true,
        "where condition:command",
        "Filter out rows from input based on condition",
        Some(r#"    The columns of the row are exported to the environment using the
    column names.

    Example:

    ps | where {$status != "Sleeping"}"#))?;
    env.declare_command(
        "sort",sort::perform, true,
        "sort column:field", "Sort input based on column", example!("ps | sort ^cpu"))?;
    env.declare_command(
        "reverse", reverse::reverse, true,
        "reverse", "Reverses the order of the rows in the input", None)?;
    env.declare_command(
        "group",group::perform, true,
        "group group=field|string", "Group input by the specified column", None)?;
    env.declare_command(
        "join", join::perform, true,
    "join left:field right:field", "Join two streams together on the specified keys", None)?;
    env.declare_command(
        "uniq", uniq::uniq, true,
        "uniq column:field",
        "Only output the first row if multiple rows has the same value for the specified column",
        example!("ps | uniq ^user"))?;
    //env.declare_str("aggr", Value::Command(CrushCommand::command_undocumented(aggr::perform)))?;
    env.declare_command(
        "count",count::perform, true,
        "count",
        "Count the number of rows in the input", example!("ps | count"))?;
    env.declare_command(
        "sum", sum_avg::sum, true,
        "sum column:field",
        "Calculate the sum for the specific column across all rows",
        example!("ps | sum ^cpu"))?;
    env.declare_command(
        "min", sum_avg::min, true,
        "min [column:field]",
        "Find the minimum value of the specific column across all rows",
        example!("ps | min ^cpu"))?;
    env.declare_command(
        "max", sum_avg::max, true,
        "max [column:field]",
        "Find the maximum value of the specific column across all rows",
        example!("ps | max ^cpu"))?;
    env.declare_command(
        "avg",sum_avg::avg, true,
        "avg column:field",
        "Calculate the average of the specific column across all rows",
        example!("ps | sum ^cpu"))?;
    env.declare_command(
        "select", select::select, true,
        "select copy_fields:field... [%] new_field=definition:command",
        "Pass on some old fields and calculate new ones for each line of input",
        example!(r#"ls | select ^user path={"{}/{}":format (pwd) file}"#))?;
    env.declare_command(
        "enumerate",enumerate::perform, true,
        "enumerate", "Prepend a column containing the row number to each row of the input", None)?;
    env.declare_command(
        "zip", zip::zip, true,
        "zip stream1:(table_stream|table|list|dict) stream2:(table_stream|table|list|dict)",
        "combine to streams of data into one", None)?;
    env.declare_command(
        "seq", seq::perform, true,
        "seq lines:integer",
        "Return a stream of numbers",
        None)?;
    env.readonly();
    Ok(())
}
