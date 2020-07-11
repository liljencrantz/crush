use crate::{
    lang::errors::argument_error,
    lang::stream::OutputStream,
};
use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::{argument::Argument, table::Row};
use crate::lang::errors::{CrushResult, error};
use crate::lang::stream::Readable;
use crate::lang::table::{ColumnType, ColumnVec};
use signature::signature;
use crate::lang::argument::ArgumentHandler;
use crate::lang::value::Field;

#[signature(
sort,
can_block = false,
short = "Sort input based on column",
long = "    ps | sort ^cpu")]
pub struct Sort {
    #[description("the column to sort on.")]
    field: Field,
}

pub fn run(idx: usize, input: &mut dyn Readable, output: OutputStream) -> CrushResult<()> {
    let mut res: Vec<Row> = Vec::new();
    while let Ok(row) = input.read() {
        res.push(row);
    }

    res.sort_by(|a, b|
        a.cells()[idx]
            .partial_cmp(&b.cells()[idx])
            .expect("OH NO!"));

    for row in res {
        output.send(row)?;
    }

    Ok(())
}

pub fn sort(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => {
            let output = context.output.initialize(input.types().to_vec())?;
            let cfg: Sort = Sort::parse(context.arguments, &context.printer)?;
            let idx = input.types().find(&cfg.field)?;
            if input.types()[idx].cell_type.is_comparable() {
                run(idx, input.as_mut(), output)
            } else {
                argument_error("Bad comparison key")
            }
        }
        None => error("Expected a stream"),
    }
}
