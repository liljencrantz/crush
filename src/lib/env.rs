use crate::lib::ExecutionContext;
use crate::errors::CrushResult;
use crate::data::{ColumnType, ValueType, Row, Value};
use std::collections::HashMap;

pub fn perform(context: ExecutionContext) -> CrushResult<()> {
    let output = context.output.initialize(vec![
        ColumnType::named("name", ValueType::Text),
        ColumnType::named("type", ValueType::Text),
    ])?;

    let mut vals : HashMap<String, ValueType> = HashMap::new();
    context.env.dump(&mut vals);

    let mut keys = vals.keys().collect::<Vec<&String>>();
    keys.sort();

    for k in keys {
        output.send(Row::new(vec![
            Value::Text(k.clone().into_boxed_str()),
            Value::Text(vals[k].to_string().into_boxed_str())
        ]));
    }

    Ok(())
}
