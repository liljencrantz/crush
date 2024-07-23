use crate::lang::errors::{error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::value::Value;
use std::collections::HashSet;
use signature::signature;
use crate::lang::command::OutputType::Passthrough;

#[signature(
    stream.uniq,
    output = Passthrough,
    short = "Only output the first row if multiple rows has the same value for the specified column",
    example = "host:procs | sort user | uniq user")]
pub struct Uniq {
    field: Option<String>,
}

pub fn uniq(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let cfg: Uniq = Uniq::parse(context.arguments, &context.global_state.printer())?;
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
        _ => error("Expected io to be a stream"),
    }
}
