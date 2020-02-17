use crate::lib::ExecutionContext;
use crate::errors::CrushResult;
use crate::data::{Struct, Value};
use crate::data::Argument;
use map_in_place::MapVecInPlace;

pub fn perform(mut context: ExecutionContext) -> CrushResult<()> {
    let arr: Vec<(Box<str>, Value)> = context.arguments.drain(..)
        .map(|v| (Box::from(v.name.unwrap()), v.value))
        .collect::<Vec<(Box<str>, Value)>>();
    context.output.send(
        Value::Struct(Struct::new(arr)))
}
