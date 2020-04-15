use crate::{
    lang::errors::argument_error,
    lang::stream::OutputStream,
};
use crate::lang::execution_context::{ExecutionContext, ArgumentVector};
use crate::lang::{argument::Argument, table::Row};
use crate::lang::errors::{CrushResult, error};
use crate::lang::stream::Readable;
use crate::lang::table::{ColumnType, ColumnVec};

pub struct Config {
    sort_column_idx: usize,
}

fn parse(
    mut arguments: Vec<Argument>,
    types: &[ColumnType]) -> CrushResult<Config> {
    let sort_column_idx =
        match arguments.len() {
            0 => {
                if types.len() != 1 {
                    return argument_error("No sort key specified");
                }
                0
            }
            1 => types.find(&arguments.field(0)?)?,
            _ => return argument_error("Too many arguments")
        };
    if !types[sort_column_idx].cell_type.is_comparable() {
        return argument_error("Bad comparison key");
    }
    Ok(Config { sort_column_idx })
}

pub fn run(config: Config, input: &mut dyn Readable, output: OutputStream) -> CrushResult<()> {
    let mut res: Vec<Row> = Vec::new();
    while let Ok(row) = input.read() {
        res.push(row);
    }

    res.sort_by(|a, b|
        a.cells()[config.sort_column_idx]
            .partial_cmp(&b.cells()[config.sort_column_idx])
            .expect("OH NO!"));

    for row in res {
        output.send(row)?;
    }

    Ok(())
}

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    match context.input.recv()?.readable() {
        Some(mut input) => {
            let output = context.output.initialize(input.types().to_vec())?;
            let config = parse(context.arguments, input.types())?;
            run(config, input.as_mut(), output)
        }
        None => error("Expected a stream"),
    }
}
