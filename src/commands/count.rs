use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::{
    data::{
        Row,
        CellType,
        Cell
    },
    stream::{InputStream},
};
use crate::data::ColumnType;

fn count_rows(s: &InputStream) -> Cell {
    let mut res: i128 = 0;
    loop {
        match s.recv() {
            Ok(_) => res+= 1,
            Err(_) => break,
        }
    }
    return Cell::Integer(res);
}

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(vec![ColumnType::named("count", CellType::Integer)])?;
    let input = context.input.initialize_stream()?;
    output.send(Row { cells: vec![count_rows(&input)]})
}
