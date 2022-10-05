use std::mem;
use crate::lang::errors::CrushResult;
use crate::lang::state::contexts::CommandContext;
use crate::lang::data::table::ColumnType;
use crate::lang::{data::table::Row, value::Value, value::ValueType};
use signature::signature;

#[signature(seq, can_block=true, short = "Return a stream of sequential numbers")]
#[derive(Debug)]
pub struct Seq {
    #[default(i128::max_value())]
    to: i128,
    #[default(0)]
    from: i128,
    #[default(1)]
    step: i128,
}

pub fn seq(context: CommandContext) -> CrushResult<()> {
    let mut cfg: Seq = Seq::parse(context.arguments, &context.global_state.printer())?;
    let output = context
        .output
        .initialize(&[ColumnType::new("value", ValueType::Integer)])?;

    if (cfg.to > cfg.from) != (cfg.step > 0) {
        mem::swap(&mut cfg.to, &mut cfg.from);
    }

    let mut idx = cfg.from;
    loop {
        if cfg.step > 0 {
            if idx >= cfg.to {
                break;
            }
        } else if idx <= cfg.to {
            break;
        }
        output.send(Row::new(vec![Value::Integer(idx)]))?;
        idx += cfg.step;
    }
    Ok(())
}
