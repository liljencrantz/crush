use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::value::Value;
use std::collections::HashSet;
use signature::signature;
use crate::lang::command::OutputType::Passthrough;
use crate::lang::value::Symbol;

#[signature(
uniq,
output = Passthrough,
short = "Only output the first row if multiple rows has the same value for the specified column",
example = "ps | uniq user")]
pub struct Uniq {
    field: Option<Symbol>,
}

pub fn uniq(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let cfg: Uniq = Uniq::parse(context.arguments, &context.global_state.printer())?;
            let output = context.output.initialize(input.types().to_vec())?;
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
