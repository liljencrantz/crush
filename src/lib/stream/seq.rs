use crate::lang::execution_context::{ExecutionContext};
use crate::lang::errors::{CrushResult};
use crate::{
    lang::{
        table::Row,
        value::ValueType,
        value::Value
    }
};
use crate::lang::table::ColumnType;
use signature::signature;
use crate::lang::argument::ArgumentHandler;

#[signature(seq, short="Return a stream of sequential numbers")]
#[derive(Debug)]
pub struct Seq {
    #[default(i128::max_value())]
    to: i128,
    #[default(0)]
    from: i128,
    #[default(1)]
    step: i128,
}

pub fn seq(context: ExecutionContext) -> CrushResult<()> {
    let mut cfg: Seq = Seq::parse(context.arguments, &context.printer)?;
    let output = context.output.initialize(vec![
        ColumnType::new("value", ValueType::Integer)])?;

    if (cfg.to>cfg.from) != (cfg.step > 0) {
        let tmp = cfg.to;
        cfg.to = cfg.from;
        cfg.from = tmp;
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
