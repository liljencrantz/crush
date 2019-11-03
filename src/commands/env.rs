use crate::commands::CompileContext;
use crate::errors::JobResult;
use crate::data::{ColumnType, CellType, Row, Cell};
use std::collections::HashMap;

pub fn compile_and_run(context: CompileContext) -> JobResult<()> {
    let output = context.output.initialize(vec![
        ColumnType::named("name", CellType::Text),
        ColumnType::named("type", CellType::Text),
    ])?;

    let mut vals : HashMap<String, CellType> = HashMap::new();
    context.env.dump(&mut vals);

    let mut keys = vals.keys().collect::<Vec<&String>>();
    keys.sort();

    for k in keys {
        output.send(Row {cells: vec![
            Cell::Text(k.clone().into_boxed_str()),
            Cell::Text(vals[k].to_string().into_boxed_str())
        ]});
    }

    Ok(())
}
