use crate::lang::command::OutputType::Passthrough;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::errors::{CrushResult, error};
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

pub fn uniq(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let cfg = Uniq::parse(context.arguments, &context.global_state.printer())?;
            let output = context.output.initialize(input.types())?;
            match cfg.field.map(|f| input.types().find(&f)).transpose()? {
                None => {
                    let mut seen: HashSet<Row> = HashSet::new();
                    while let Ok(row) = input.read() {
                        if !seen.contains(&row) {
                            seen.insert(row.clone());
                            output.send(row)?;
                        }
                    }
                }
                Some(idx) => {
                    let mut seen: HashSet<Value> = HashSet::new();
                    while let Ok(row) = input.read() {
                        if !seen.contains(&row.cells()[idx]) {
                            seen.insert(row.cells()[idx].clone());
                            output.send(row)?;
                        }
                    }
                }
            }
            Ok(())
        }
        _ => error("`uniq`: Expected input to be a stream"),
    }
}
