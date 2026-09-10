use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::errors::{CrushResult, command_error};
use crate::lang::state::contexts::CommandContext;
use crate::lang::value::Value;
use signature::signature;
use std::collections::HashSet;

#[signature(
    stream.uniq,
    output = Passthrough,
    short = "Only output the first row whenever multiple rows has the same value for the specified column",
    long = "If no column is given, the entire rows are compared.",
    long = "",
    long = "This command does not just remove consecutive repeated column values, any repeated column values over the entire stream are removed.",
    example = "host:procs | uniq user")]
pub struct Uniq {
    #[description("The field to compare.")]
    field: Option<String>,
}

pub fn uniq(mut context: CommandContext) -> CrushResult<()> {
    let mut input = context.input_stream()?;
    let cfg = Uniq::parse(context.remove_arguments(), &context.global_state.printer())?;
    let output = context.initialize_output(input.types())?;
    match cfg.field.map(|f| input.types().find(&f)).transpose()? {
        None => {
            let mut seen: HashSet<Row> = HashSet::new();
            while let Some(row) = input.next_row()? {
                // A column's *declared* type can be `$any` (e.g. any closure-computed
                // `select` column), which is always hashable -- the actual value only
                // exists at runtime, so this has to be checked per row rather than once
                // up front, the same way sum/avg handle `$any` columns.
                for cell in row.cells() {
                    if !cell.value_type().is_hashable() {
                        return command_error(format!(
                            "Can't deduplicate whole rows: encountered a value of type `{}`, which is not hashable.",
                            cell.value_type(),
                        ));
                    }
                }
                if !seen.contains(&row) {
                    seen.insert(row.clone());
                    output.send(row)?;
                }
            }
        }
        Some(idx) => {
            let mut seen: HashSet<Value> = HashSet::new();
            while let Some(row) = input.next_row()? {
                if !row.cells()[idx].value_type().is_hashable() {
                    return command_error(format!(
                        "Can't deduplicate on column `{}`: encountered a value of type `{}`, which is not hashable.",
                        input.types()[idx].name(),
                        row.cells()[idx].value_type(),
                    ));
                }
                if !seen.contains(&row.cells()[idx]) {
                    seen.insert(row.cells()[idx].clone());
                    output.send(row)?;
                }
            }
        }
    }
    Ok(())
}
