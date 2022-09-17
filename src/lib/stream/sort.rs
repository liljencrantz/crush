use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{error, CrushResult};
use crate::lang::state::contexts::CommandContext;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::errors::argument_error_legacy;
use signature::signature;
use std::cmp::Ordering;

#[signature(
sort,
short = "Sort input based on column",
example = "host:procs | sort cpu",
output = Passthrough)]
pub struct Sort {
    #[unnamed()]
    #[description("the columns to sort on. Optional if input only has one column.")]
    field: Vec<String>,
    #[description("reverse the sort order.")]
    #[default(false)]
    reverse: bool,
}

fn sort(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream()? {
        Some(mut input) => {
            let output = context.output.initialize(input.types().to_vec())?;
            let cfg: Sort = Sort::parse(context.arguments, &context.global_state.printer())?;
            let indices = if cfg.field.is_empty() {
                if input.types().len() == 1 {
                    vec![0]
                } else {
                    return argument_error_legacy("Missing comparison key");
                }
            } else {
                cfg.field.iter().map(|f| input.types().find(f)).collect::<CrushResult<Vec<_>>>()?
            };

            for idx in &indices {
                if !input.types()[*idx].cell_type.is_comparable() {
                    return argument_error_legacy("Bad comparison key");
                }
            }

            let mut res: Vec<Row> = Vec::new();

            while let Ok(row) = input.read() {
                res.push(row);
            }

            if cfg.reverse {
                res.sort_by(|a, b| {
                    for idx in &indices {
                        match b.cells()[*idx].partial_cmp(&a.cells()[*idx]) {
                            None => panic!("Unexpcted sort failure"),
                            Some(Ordering::Equal) => {}
                            Some(ordering) => return ordering,
                        }
                    }
                    Ordering::Equal
                });
            } else {
                res.sort_by(|b, a| {
                    for idx in &indices {
                        match b.cells()[*idx].partial_cmp(&a.cells()[*idx]) {
                            None => panic!("Unexpcted sort failure"),
                            Some(Ordering::Equal) => {}
                            Some(ordering) => return ordering,
                        }
                    }
                    Ordering::Equal
                });
            }

            for row in res {
                output.send(row)?;
            }

            Ok(())
        }
        None => error("Expected a stream"),
    }
}
