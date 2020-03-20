use crate::lang::command::ExecutionContext;
use crate::lang::errors::CrushResult;
use crate::lang::{table::ColumnType, value::ValueType, table::Row, value::Value};
use std::collections::HashMap;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![
        ColumnType::new("name", ValueType::String),
        ColumnType::new("type", ValueType::String),
    ])?;

    let mut vals : HashMap<String, ValueType> = HashMap::new();
    context.env.dump(&mut vals);

    let mut keys = vals.keys().collect::<Vec<&String>>();
    keys.sort();

    for k in keys {
        output.send(Row::new(vec![
            Value::String(k.clone().into_boxed_str()),
            Value::String(vals[k].to_string().into_boxed_str())
        ]));
    }

    Ok(())
}
