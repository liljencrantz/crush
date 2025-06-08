use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::data::table::ColumnType;
use crate::lang::{data::table::Row, value::Value, value::ValueType};
use signature::signature;

#[signature(
    stream.seq,
    can_block=true,
    short = "Return a stream of sequential numbers"
)]
#[derive(Debug)]
pub struct Seq {
    #[description("the first number in the sequence.")]
    #[default(0)]
    from: i128,
    #[description("the step size.")]
    #[default(1)]
    step: i128,
    #[description("the end of the sequence (exclusive). If not specified, the sequence will continue forever.")]
    to: Option<i128>,
}

pub fn seq(context: CommandContext) -> CrushResult<()> {
    let mut cfg = Seq::parse(context.arguments, &context.global_state.printer())?;
    let output = context
        .output
        .initialize(&[ColumnType::new("value", ValueType::Integer)])?;
    
    let mut idx = cfg.from;
    loop {
        if let Some(to) = cfg.to {
            if cfg.step > 0 {
                if idx >= to {
                    break;
                }
            } else if idx <= to {
                break;
            }
        }
        output.send(Row::new(vec![Value::Integer(idx)]))?;
        idx += cfg.step;
    }
    Ok(())
}
