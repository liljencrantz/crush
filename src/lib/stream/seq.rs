use crate::lang::ExecutionContext;
use crate::errors::{CrushResult, error};
use crate::{
    lang::{
        Row,
        ValueType,
        Value
    }
};
use crate::lang::{ColumnType, Argument, RowsReader};
use crate::lib::command_util::find_field_from_str;
use crate::stream::{Readable};
use crate::lib::parse_util::single_argument_integer;

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let c  =single_argument_integer(context.arguments)?;
    let output = context.output.initialize(vec![
        ColumnType::named("value", ValueType::Integer)])?;

    for i in 0..c {
        output.send(Row::new(vec![Value::Integer(i)]))?;
    }
    Ok(())
}
