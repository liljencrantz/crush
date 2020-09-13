use crate::lang::command::OutputType::Passthrough;
use crate::lang::errors::{error, CrushResult};
use crate::lang::execution_context::CommandContext;
use crate::lang::data::table::ColumnVec;
use crate::lang::data::table::Row;
use crate::lang::value::Field;
use crate::lang::errors::argument_error_legacy;
use signature::signature;

#[signature(
    sort,
    can_block=true,
    short="Sort io based on column",
    long="ps | sort ^cpu",
    output=Passthrough)]
pub struct Sort {
    #[description("the column to sort on. Not required if there is only one column.")]
    field: Option<Field>,
    #[description("if true, reverse sort order.")]
    #[default(false)]
    reverse: bool,
}

fn sort(context: CommandContext) -> CrushResult<()> {
    match context.input.recv()?.stream() {
        Some(mut input) => {
            let output = context.output.initialize(input.types().to_vec())?;
            let cfg: Sort = Sort::parse(context.arguments, &context.printer)?;
            let idx = match cfg.field {
                None => {
                    if input.types().len() == 1 {
                        0
                    } else {
                        return argument_error_legacy("Missing comparison key");
                    }
                }
                Some(field) => input.types().find(&field)?,
            };

            if input.types()[idx].cell_type.is_comparable() {
                let mut res: Vec<Row> = Vec::new();

                while let Ok(row) = input.read() {
                    res.push(row);
                }

                if cfg.reverse {
                    res.sort_by(|a, b|
                        b.cells()[idx].partial_cmp(&a.cells()[idx])
                            .expect("OH NO!"));
                } else {
                    res.sort_by(|a, b|
                        a.cells()[idx].partial_cmp(&b.cells()[idx])
                            .expect("OH NO!"));
                }

                for row in res {
                    output.send(row)?;
                }

                Ok(())
            } else {
                argument_error_legacy("Bad comparison key")
            }
        }
        None => error("Expected a stream"),
    }
}
