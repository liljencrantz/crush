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
mod sum_avg;
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
    env.declare("where", Value::Command(CrushCommand::command(
        r#where::perform, true,
        "where condition:command",
        "Filter out rows from input based on condition",
        Some(r#"    The columns of the row are exported to the environment using the
    column names.

    Example:

    ps | where {$status != "Sleeping"}"#))))?;
    env.declare("sort", Value::Command(CrushCommand::command(
        sort::perform, true,
        "sort column:field", "Sort input based on column", example!("ps | sort ^cpu"))))?;
    env.declare("reverse", Value::Command(CrushCommand::command(
        reverse::perform, true,
        "reverse", "Reverses the order of the rows in the input", None)))?;
    env.declare("group", Value::Command(CrushCommand::command_undocumented(group::perform, true)))?;
    env.declare("join", Value::Command(CrushCommand::command_undocumented(join::perform, true)))?;
    env.declare("uniq", Value::Command(CrushCommand::command(
        uniq::perform, true,
        "uniq column:field",
        "Only output the first row if multiple rows has the same value for the specified column",
        example!("ps | uniq ^user"))))?;
    //env.declare_str("aggr", Value::Command(CrushCommand::command_undocumented(aggr::perform)))?;
    env.declare("count", Value::Command(CrushCommand::command(
        count::perform, true,
        "count",
        "Count the number of rows in the input", example!("ps | count"))))?;
    env.declare("sum", Value::Command(CrushCommand::command(
        sum_avg::sum, true,
        "sum column:field",
        "Calculate the sum for the specific column across all rows",
        example!("ps | sum ^cpu"))))?;
    env.declare("avg", Value::Command(CrushCommand::command(
        sum_avg::avg, true,
        "avg column:field",
        "Calculate the average of the specific column across all rows",
        example!("ps | sum ^cpu"))))?;
    env.declare("select", Value::Command(CrushCommand::command(
        select::perform, true,
        "select copy_fields:field... [%] new_field=definition:command",
        "Pass on some old fields and calculate new ones for each line of input",
        example!(r#"ls | select ^user path={"{}/{}":format (pwd) file}"#))))?;
    env.declare("enumerate", Value::Command(CrushCommand::command(
        enumerate::perform, true,
        "enumerate", "Prepend a column containing the row number to each row of the input", None)))?;
    env.declare("zip", Value::Command(CrushCommand::command(
        zip::perform, true,
        "zip stream1:(table_stream|table|list|dict) stream2:(table_stream|table|list|dict)",
        "combine to streams of data into one", None)))?;
    env.declare("seq", Value::Command(CrushCommand::command(
        seq::perform, true,
        "seq lines:integer",
        "Return a stream of numbers",
        None)))?;
    env.readonly();
    Ok(())
}
