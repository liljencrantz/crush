use crate::lang::command::{ExecutionContext, ArgumentVector};
use crate::lang::errors::{CrushResult, error};
use crate::{
    lang::{
        table::Row,
        value::ValueType,
        value::Value
    }
};
use crate::lang::{table::ColumnType, argument::Argument, table::TableReader};
use crate::lib::command_util::find_field_from_str;
use crate::lang::stream::{Readable};

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let c  = context.arguments.integer(0)?;
    let output = context.output.initialize(vec![
        ColumnType::new("value", ValueType::Integer)])?;

    for i in 0..c {
        output.send(Row::new(vec![Value::Integer(i)]))?;
    }
    Ok(())
}
